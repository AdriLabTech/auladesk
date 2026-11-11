//! Cuentas vinculadas (una por plataforma) y alumnado de las cuentas familiares.
//! Nunca se guardan credenciales: solo nombre visible, estado de la sesión y los datos sincronizados.
//! Regla dura: todo dato de Familias lleva `student_id`; aquí se valida que un alumno pertenece a su cuenta.
use crate::error::{AppError, AppResult};
use serde::Serialize;
use sqlx::SqlitePool;

/// La cuenta Moodle es única y fija (la crea la migración 0007).
pub const MOODLE_ACCOUNT: i64 = 1;

pub const PLATFORMS: [&str; 2] = ["moodle", "familias"];
pub const STATES: [&str; 4] = ["connected", "expired", "disconnected", "unavailable"];

fn now() -> i64 { chrono::Utc::now().timestamp() }

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Account {
    pub id: i64, pub platform: String, pub label: String, pub external_user: Option<String>, pub state: String,
    pub last_ok_at: Option<i64>, pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Student {
    pub id: i64, pub account_id: i64, pub external_ref: String, pub display_name: String,
    pub center: Option<String>, pub course_group: Option<String>, pub tutor: Option<String>, pub last_synced_at: Option<i64>,
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<Account>> {
    Ok(sqlx::query_as("SELECT id, platform, label, external_user, state, last_ok_at, last_error FROM linked_account ORDER BY id").fetch_all(pool).await?)
}

/// Id de la cuenta de la plataforma; la crea si no existe (solo `familias`; Moodle ya existe).
pub async fn ensure(pool: &SqlitePool, platform: &str) -> AppResult<i64> {
    if !PLATFORMS.contains(&platform) { return Err(AppError::Other(format!("plataforma desconocida: {platform}"))); }
    if let Some(id) = sqlx::query_scalar::<_, i64>("SELECT id FROM linked_account WHERE platform=? ORDER BY id LIMIT 1").bind(platform).fetch_optional(pool).await? { return Ok(id); }
    let (label, state) = match platform { "familias" => ("Familias", "disconnected"), _ => ("Moodle", "disconnected") };
    Ok(sqlx::query_scalar("INSERT INTO linked_account(platform,label,state,created_at) VALUES(?,?,?,?) RETURNING id").bind(platform).bind(label).bind(state).bind(now()).fetch_one(pool).await?)
}

pub async fn set_state(pool: &SqlitePool, id: i64, state: &str, error: Option<&str>) -> AppResult<()> {
    if !STATES.contains(&state) { return Err(AppError::Other(format!("estado no válido: {state}"))); }
    let ok_at = (state == "connected").then(now);
    sqlx::query("UPDATE linked_account SET state=?, last_error=?, last_ok_at=coalesce(?, last_ok_at) WHERE id=?")
        .bind(state).bind(error.map(|e| e.chars().take(300).collect::<String>())).bind(ok_at).bind(id).execute(pool).await?;
    Ok(())
}

pub async fn set_external_user(pool: &SqlitePool, id: i64, external_user: Option<&str>) -> AppResult<()> {
    sqlx::query("UPDATE linked_account SET external_user=? WHERE id=?").bind(external_user).bind(id).execute(pool).await?;
    Ok(())
}

pub async fn students(pool: &SqlitePool, account_id: i64) -> AppResult<Vec<Student>> {
    Ok(sqlx::query_as("SELECT id, account_id, external_ref, display_name, center, course_group, tutor, last_synced_at FROM student WHERE account_id=? AND removed_at IS NULL ORDER BY display_name").bind(account_id).fetch_all(pool).await?)
}

pub struct NewStudent<'a> { pub external_ref: &'a str, pub display_name: &'a str, pub center: Option<&'a str>, pub course_group: Option<&'a str>, pub tutor: Option<&'a str> }

/// Crea o actualiza un alumno de la cuenta (clave estable `external_ref`). Devuelve su id.
pub async fn upsert_student(pool: &SqlitePool, account_id: i64, s: &NewStudent<'_>) -> AppResult<i64> {
    let t = now();
    Ok(sqlx::query_scalar(
        "INSERT INTO student(account_id,external_ref,display_name,center,course_group,tutor,first_seen_at,last_synced_at) VALUES(?,?,?,?,?,?,?,?)
         ON CONFLICT(account_id,external_ref) DO UPDATE SET display_name=excluded.display_name, center=coalesce(excluded.center, student.center), course_group=coalesce(excluded.course_group, student.course_group), tutor=coalesce(excluded.tutor, student.tutor), last_synced_at=excluded.last_synced_at, removed_at=NULL
         RETURNING id")
        .bind(account_id).bind(s.external_ref).bind(s.display_name).bind(s.center).bind(s.course_group).bind(s.tutor).bind(t).bind(t).fetch_one(pool).await?)
}

/// Marca como retirados los alumnos de la cuenta que ya no aparecen (se conservan sus datos).
pub async fn retire_missing(pool: &SqlitePool, account_id: i64, keep_refs: &[String]) -> AppResult<i64> {
    let all: Vec<(i64, String)> = sqlx::query_as("SELECT id, external_ref FROM student WHERE account_id=? AND removed_at IS NULL").bind(account_id).fetch_all(pool).await?;
    let mut n = 0;
    for (id, r) in all.into_iter().filter(|(_, r)| !keep_refs.contains(r)) {
        let _ = r;
        n += sqlx::query("UPDATE student SET removed_at=? WHERE id=?").bind(now()).bind(id).execute(pool).await?.rows_affected() as i64;
    }
    Ok(n)
}

/// Comprueba que el alumno existe, no está retirado y es de esa cuenta. Toda consulta de datos de Familias pasa por aquí.
pub async fn check_student(pool: &SqlitePool, account_id: i64, student_id: i64) -> AppResult<()> {
    let ok: Option<i64> = sqlx::query_scalar("SELECT id FROM student WHERE id=? AND account_id=? AND removed_at IS NULL").bind(student_id).bind(account_id).fetch_optional(pool).await?;
    ok.map(|_| ()).ok_or_else(|| AppError::Other("El alumno no pertenece a esta cuenta.".into()))
}

/// Desvincular: borra todo lo de la cuenta (alumnado, calificaciones, faltas, mensajes, eventos y avisos, por cascada),
/// el nombre visible y deja la cuenta desconectada. Las cookies y el llavero los borra quien llama.
pub async fn purge_data(pool: &SqlitePool, account_id: i64) -> AppResult<()> {
    if account_id == MOODLE_ACCOUNT { return Err(AppError::Other("Los datos de Moodle no se borran desde aquí.".into())); }
    let mut tx = pool.begin().await?;
    for t in ["notification", "calendar_event", "school_message", "integration_sync_history"] {
        sqlx::query(&format!("DELETE FROM {t} WHERE account_id=?")).bind(account_id).execute(&mut *tx).await?;
    }
    sqlx::query("DELETE FROM student WHERE account_id=?").bind(account_id).execute(&mut *tx).await?;   // en cascada: grade_record, attendance_record
    sqlx::query("UPDATE linked_account SET state='disconnected', external_user=NULL, last_error=NULL, last_ok_at=NULL WHERE id=?").bind(account_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::db;

    // DATOS SINTÉTICOS. Dos alumnos ficticios con el mismo id externo de registro en cuentas distintas.
    pub async fn setup() -> (tempfile::TempDir, SqlitePool, i64, i64, i64) {
        let d = tempfile::tempdir().unwrap();
        let p = db::open(&d.path().join("t.db")).await.unwrap();
        let acc = ensure(&p, "familias").await.unwrap();
        let a = upsert_student(&p, acc, &NewStudent { external_ref: "ref-a", display_name: "Alumno A", center: Some("IES X"), course_group: Some("1.º A"), tutor: None }).await.unwrap();
        let b = upsert_student(&p, acc, &NewStudent { external_ref: "ref-b", display_name: "Alumno B", center: None, course_group: None, tutor: None }).await.unwrap();
        (d, p, acc, a, b)
    }

    #[tokio::test]
    async fn accounts_are_created_once_and_states_validated() {
        let (_d, p, acc, _, _) = setup().await;
        assert_eq!(ensure(&p, "familias").await.unwrap(), acc, "una sola cuenta por plataforma");
        assert_eq!(ensure(&p, "moodle").await.unwrap(), MOODLE_ACCOUNT);
        assert!(ensure(&p, "otra").await.is_err());
        set_state(&p, acc, "connected", None).await.unwrap();
        assert!(set_state(&p, acc, "inventado", None).await.is_err());
        let a = list(&p).await.unwrap();
        assert_eq!(a.iter().find(|x| x.id == acc).unwrap().state, "connected");
        assert!(a.iter().find(|x| x.id == acc).unwrap().last_ok_at.is_some());
    }

    #[tokio::test]
    async fn students_are_upserted_and_retired_not_deleted() {
        let (_d, p, acc, a, _) = setup().await;
        let again = upsert_student(&p, acc, &NewStudent { external_ref: "ref-a", display_name: "Alumno A (nuevo nombre)", center: None, course_group: None, tutor: Some("Tutora") }).await.unwrap();
        assert_eq!(again, a, "mismo alumno = misma fila");
        assert_eq!(students(&p, acc).await.unwrap().len(), 2);
        assert_eq!(retire_missing(&p, acc, &["ref-a".into()]).await.unwrap(), 1);
        assert_eq!(students(&p, acc).await.unwrap().len(), 1);
        assert!(check_student(&p, acc, a).await.is_ok());
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM student").fetch_one(&p).await.unwrap();
        assert_eq!(n, 2, "el alumno retirado se conserva");
    }

    #[tokio::test]
    async fn a_student_must_belong_to_the_account() {
        let (_d, p, acc, a, _) = setup().await;
        sqlx::query("INSERT INTO linked_account(id,platform,label,external_user,created_at) VALUES(9,'familias','Otra','x',0)").execute(&p).await.unwrap();
        assert!(check_student(&p, 9, a).await.is_err(), "alumno de otra cuenta");
        assert!(check_student(&p, acc, 9999).await.is_err());
    }

    #[tokio::test]
    async fn disconnect_purges_only_that_accounts_data_and_never_moodles() {
        let (_d, p, acc, a, _) = setup().await;
        sqlx::query("INSERT INTO grade_record(student_id,external_id,subject,evaluation,grade_text,content_hash,first_seen_at,last_synced_at) VALUES(?,?,?,?,?,?,0,0)").bind(a).bind("g1").bind("Mates").bind("1.ª").bind("7").bind("h").execute(&p).await.unwrap();
        sqlx::query("INSERT INTO school_message(account_id,student_id,external_id,kind,subject,content_hash,first_seen_at,last_synced_at) VALUES(?,?,'m1','mensaje','Hola','h',0,0)").bind(acc).bind(a).execute(&p).await.unwrap();
        sqlx::query("INSERT INTO calendar_event(source,account_id,student_id,external_id,kind,title,starts_at) VALUES('familias',?,?,'e1','tutoring','t',1)").bind(acc).bind(a).execute(&p).await.unwrap();
        sqlx::query("INSERT INTO notification(category,title,created_at,account_id) VALUES('grade','x',0,?)").bind(acc).execute(&p).await.unwrap();
        sqlx::query("INSERT INTO calendar_event(source,account_id,kind,title,starts_at,moodle_event_id) VALUES('moodle',1,'due','Entrega',1,5)").execute(&p).await.unwrap();
        purge_data(&p, acc).await.unwrap();
        for t in ["grade_record", "school_message", "student"] {
            let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {t}")).fetch_one(&p).await.unwrap();
            assert_eq!(n, 0, "{t}");
        }
        let ev: i64 = sqlx::query_scalar("SELECT count(*) FROM calendar_event").fetch_one(&p).await.unwrap();
        assert_eq!(ev, 1, "el evento de Moodle sigue ahí");
        let nt: i64 = sqlx::query_scalar("SELECT count(*) FROM notification").fetch_one(&p).await.unwrap();
        assert_eq!(nt, 0);
        assert_eq!(list(&p).await.unwrap().iter().find(|x| x.id == acc).unwrap().state, "disconnected");
        assert!(purge_data(&p, MOODLE_ACCOUNT).await.is_err());
    }
}
