//! Acciones de escritura en el portal de Familias. Reglas duras (decididas por Adrián para Moodle y extendidas aquí):
//! 1) nada se envía sin confirmación explícita del usuario en cada acción; 2) lo que se confirma es exactamente lo que se envía
//! (los parámetros quedan fijados al preparar); 3) cada intento queda registrado; 4) sin receta verificada contra el portal la acción
//! NO se simula: queda `unsupported` con su motivo. Hoy no hay ninguna receta de escritura verificada, así que ninguna acción llega al portal.
use crate::accounts;
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

pub const KINDS: [&str; 3] = ["justify_absence", "request_tutoring", "send_message"];
pub const UNSUPPORTED_REASON: &str = "Acción no disponible: todavía no se ha verificado cómo la admite el portal (hace falta el reconocimiento con la cuenta de la familia). No se ha enviado nada.";

fn now() -> i64 { chrono::Utc::now().timestamp() }

#[derive(Debug, Deserialize)]
pub struct Params { pub attendance_id: Option<i64>, pub reason: Option<String>, pub subject: Option<String>, pub body: Option<String> }

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ActionRow { pub id: i64, pub student_id: i64, pub kind: String, pub target_id: Option<i64>, pub params_json: String, pub state: String, pub created_at: i64, pub confirmed_at: Option<i64>, pub finished_at: Option<i64>, pub error: Option<String> }

fn text(s: &Option<String>, min: usize, max: usize, what: &str) -> AppResult<String> {
    let t = super::mapper::clean(s.as_deref().unwrap_or(""), max, true);
    if t.chars().count() < min { return Err(AppError::Other(format!("{what}: escribe al menos {min} caracteres."))); }
    Ok(t)
}

/// Valida y registra la acción como `prepared`. No toca el portal.
pub async fn prepare(pool: &SqlitePool, account: i64, student: i64, kind: &str, p: &Params) -> AppResult<i64> {
    accounts::check_student(pool, account, student).await?;
    if !KINDS.contains(&kind) { return Err(AppError::Other(format!("acción desconocida: {kind}"))); }
    let (target, params) = match kind {
        "justify_absence" => {
            let aid = p.attendance_id.ok_or_else(|| AppError::Other("Elige la falta que quieres justificar.".into()))?;
            let rec: Option<(i64, i64)> = sqlx::query_as("SELECT justified, 1 FROM attendance_record WHERE id=? AND student_id=? AND removed_at IS NULL").bind(aid).bind(student).fetch_optional(pool).await?;
            match rec { None => return Err(AppError::Other("Esa falta no es de este alumno.".into())), Some((1, _)) => return Err(AppError::Other("Esa falta ya consta como justificada.".into())), _ => {} }
            let busy: i64 = sqlx::query_scalar("SELECT count(*) FROM family_action WHERE kind='justify_absence' AND target_id=? AND state IN ('prepared','running')").bind(aid).fetch_one(pool).await?;
            if busy > 0 { return Err(AppError::Other("Ya hay una justificación pendiente para esa falta.".into())); }
            (Some(aid), serde_json::json!({ "reason": text(&p.reason, 3, 1000, "Motivo")? }))
        }
        "request_tutoring" => (None, serde_json::json!({ "reason": text(&p.reason, 3, 1000, "Motivo")? })),
        _ => (None, serde_json::json!({ "subject": text(&p.subject, 2, 200, "Asunto")?, "body": text(&p.body, 2, 5000, "Mensaje")? })),
    };
    Ok(sqlx::query_scalar("INSERT INTO family_action(account_id,student_id,kind,target_id,params_json,created_at) VALUES(?,?,?,?,?,?) RETURNING id")
        .bind(account).bind(student).bind(kind).bind(target).bind(params.to_string()).bind(now()).fetch_one(pool).await?)
}

/// Ejecuta una acción preparada. Exige `confirmed == true` y una sesión activa. Sin receta verificada → `unsupported`.
pub async fn run(pool: &SqlitePool, account: i64, id: i64, confirmed: bool, session_active: bool) -> AppResult<ActionRow> {
    if !confirmed { return Err(AppError::Other("Hace falta tu confirmación explícita para enviar esta acción al portal.".into())); }
    let a: ActionRow = sqlx::query_as("SELECT id,student_id,kind,target_id,params_json,state,created_at,confirmed_at,finished_at,error FROM family_action WHERE id=? AND account_id=?").bind(id).bind(account).fetch_optional(pool).await?
        .ok_or_else(|| AppError::Other("Acción no encontrada.".into()))?;
    if a.state != "prepared" { return Err(AppError::Other(format!("La acción ya está en estado «{}».", a.state))); }
    if !session_active { return Err(AppError::Other("No hay sesión de Familias activa.".into())); }
    sqlx::query("UPDATE family_action SET confirmed_at=? WHERE id=?").bind(now()).bind(id).execute(pool).await?;
    // Aquí iría el driver de la receta de escritura. No existe ninguno verificado: no se simula.
    sqlx::query("UPDATE family_action SET state='unsupported', finished_at=?, error=? WHERE id=?").bind(now()).bind(UNSUPPORTED_REASON).bind(id).execute(pool).await?;
    get(pool, account, id).await
}

pub async fn cancel(pool: &SqlitePool, account: i64, id: i64) -> AppResult<()> {
    let n = sqlx::query("UPDATE family_action SET state='cancelled', finished_at=? WHERE id=? AND account_id=? AND state='prepared'").bind(now()).bind(id).bind(account).execute(pool).await?.rows_affected();
    if n == 0 { return Err(AppError::Other("Solo se pueden cancelar las acciones preparadas.".into())); }
    Ok(())
}

pub async fn get(pool: &SqlitePool, account: i64, id: i64) -> AppResult<ActionRow> {
    sqlx::query_as("SELECT id,student_id,kind,target_id,params_json,state,created_at,confirmed_at,finished_at,error FROM family_action WHERE id=? AND account_id=?").bind(id).bind(account).fetch_one(pool).await.map_err(Into::into)
}

pub async fn list(pool: &SqlitePool, account: i64, student: i64) -> AppResult<Vec<ActionRow>> {
    accounts::check_student(pool, account, student).await?;
    Ok(sqlx::query_as("SELECT id,student_id,kind,target_id,params_json,state,created_at,confirmed_at,finished_at,error FROM family_action WHERE account_id=? AND student_id=? ORDER BY id DESC LIMIT 100").bind(account).bind(student).fetch_all(pool).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::tests::setup;
    // DATOS SINTÉTICOS.
    async fn absence(p: &SqlitePool, student: i64, justified: i64) -> i64 {
        sqlx::query_scalar("INSERT INTO attendance_record(student_id,external_id,day,kind,justified,content_hash,first_seen_at,last_synced_at) VALUES(?,?,?,?,?,?,0,0) RETURNING id")
            .bind(student).bind(format!("x{justified}{student}")).bind("2026-11-03").bind("falta").bind(justified).bind("h").fetch_one(p).await.unwrap()
    }
    fn pr(a: Option<i64>, r: &str) -> Params { Params { attendance_id: a, reason: Some(r.into()), subject: None, body: None } }

    #[tokio::test]
    async fn preparing_validates_and_never_reaches_the_portal() {
        let (_d, p, acc, a, b) = setup().await;
        let fa = absence(&p, a, 0).await;
        assert!(prepare(&p, acc, a, "justify_absence", &pr(Some(fa), "Cita médica")).await.is_ok());
        assert!(prepare(&p, acc, a, "justify_absence", &pr(Some(fa), "Cita médica")).await.is_err(), "ya hay una pendiente");
        assert!(prepare(&p, acc, b, "justify_absence", &pr(Some(fa), "Cita médica")).await.is_err(), "la falta es de otro alumno");
        let done = absence(&p, a, 1).await;
        assert!(prepare(&p, acc, a, "justify_absence", &pr(Some(done), "Cita médica")).await.is_err(), "ya justificada");
        assert!(prepare(&p, acc, a, "justify_absence", &pr(Some(fa + 99), "Cita")).await.is_err());
        assert!(prepare(&p, acc, a, "justify_absence", &pr(None, "Cita médica")).await.is_err());
        assert!(prepare(&p, acc, a, "request_tutoring", &pr(None, "x")).await.is_err(), "motivo demasiado corto");
        assert!(prepare(&p, acc, a, "borrar_todo", &pr(None, "Cita médica")).await.is_err());
        let m = Params { attendance_id: None, reason: None, subject: Some("Reunión".into()), body: Some("¿Podemos hablar?".into()) };
        assert!(prepare(&p, acc, a, "send_message", &m).await.is_ok());
    }

    #[tokio::test]
    async fn running_needs_explicit_confirmation_and_is_honest_about_support() {
        let (_d, p, acc, a, _) = setup().await;
        let fa = absence(&p, a, 0).await;
        let id = prepare(&p, acc, a, "justify_absence", &pr(Some(fa), "Cita médica")).await.unwrap();
        assert!(run(&p, acc, id, false, true).await.is_err(), "sin confirmar no se hace nada");
        assert!(run(&p, acc, id, true, false).await.is_err(), "sin sesión no se hace nada");
        assert_eq!(get(&p, acc, id).await.unwrap().state, "prepared");
        let r = run(&p, acc, id, true, true).await.unwrap();
        assert_eq!(r.state, "unsupported", "sin receta verificada no se simula el envío");
        assert!(r.error.unwrap().contains("No se ha enviado nada"));
        assert!(r.confirmed_at.is_some());
        assert!(run(&p, acc, id, true, true).await.is_err(), "no se reejecuta");
        let j: i64 = sqlx::query_scalar("SELECT justified FROM attendance_record WHERE id=?").bind(fa).fetch_one(&p).await.unwrap();
        assert_eq!(j, 0, "los datos locales no cambian como si se hubiera justificado");
    }

    #[tokio::test]
    async fn only_prepared_actions_can_be_cancelled_and_the_log_is_kept() {
        let (_d, p, acc, a, _) = setup().await;
        let id = prepare(&p, acc, a, "request_tutoring", &pr(None, "Hablar de las notas")).await.unwrap();
        cancel(&p, acc, id).await.unwrap();
        assert!(cancel(&p, acc, id).await.is_err());
        assert_eq!(list(&p, acc, a).await.unwrap().len(), 1, "queda en el registro");
    }
}
