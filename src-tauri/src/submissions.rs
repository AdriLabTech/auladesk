//! Entregas: preparación local versionada, inicio del proceso en Moodle y comprobación verificable.
//! Regla central: una versión solo pasa a «confirmed» si el archivo que Moodle muestra tiene el MISMO SHA-256 que el preparado.
//! AulaDesk nunca pulsa «Enviar»: el envío lo hace el usuario en la propia web de Moodle.
use crate::downloads::{hash_and_head, sanitize_filename, section_dir};
use crate::error::{AppError, AppResult};
use crate::paths::safe_join;
use serde::Serialize;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, sqlx::FromRow, Clone)]
pub struct VersionRow {
    pub id: i64, pub submission_id: i64, pub activity_id: i64, pub activity_name: String, pub course_name: String, pub version: i64, pub state: String,
    pub file_id: i64, pub file_name: String, pub rel_path: String, pub sha256: String, pub size_bytes: i64,
    pub created_at: i64, pub sent_at: Option<i64>, pub confirmed_at: Option<i64>, pub confirmation_evidence: Option<String>,
    pub grade: Option<String>, pub teacher_feedback: Option<String>, pub error_detail: Option<String>, pub deadline_at: Option<i64>,
}

const SELECT: &str = "SELECT v.id, v.submission_id, su.activity_id, a.name AS activity_name, c.fullname AS course_name, v.version, v.state, v.file_id, f.original_name AS file_name, f.rel_path, v.sha256, f.size_bytes,
    su.created_at, v.sent_at, v.confirmed_at, v.confirmation_evidence, v.grade, v.teacher_feedback, v.error_detail, su.deadline_at
    FROM submission_version v JOIN submission su ON su.id=v.submission_id JOIN activity a ON a.id=su.activity_id JOIN course c ON c.id=a.course_id JOIN local_file f ON f.id=v.file_id";

pub async fn get(pool: &SqlitePool, id: i64) -> AppResult<VersionRow> {
    sqlx::query_as(&format!("{SELECT} WHERE v.id=?")).bind(id).fetch_optional(pool).await?.ok_or_else(|| AppError::Other("versión de entrega desconocida".into()))
}

pub async fn history(pool: &SqlitePool, activity_id: Option<i64>) -> AppResult<Vec<VersionRow>> {
    Ok(sqlx::query_as(&format!("{SELECT} WHERE (?1 IS NULL OR su.activity_id=?1) ORDER BY v.id DESC")).bind(activity_id).fetch_all(pool).await?)
}

/// Copia `src` a `files_root/<curso>/<tema>/Entregas/` (nunca sobrescribe) y crea la versión «prepared».
pub async fn prepare(pool: &SqlitePool, files_root: &Path, activity_id: i64, src: &Path) -> AppResult<VersionRow> {
    let loc: (i64, i64, String, i64, Option<i64>, String) = sqlx::query_as(
        "SELECT c.id, s.id, c.folder_name, s.position, a.due_at, a.modname FROM activity a JOIN course c ON c.id=a.course_id JOIN course_section s ON s.id=a.section_id WHERE a.id=?")
        .bind(activity_id).fetch_optional(pool).await?.ok_or_else(|| AppError::Other("actividad desconocida".into()))?;
    if loc.5 != "assign" { return Err(AppError::Other("solo las tareas admiten entregas".into())); }
    if !src.is_file() { return Err(AppError::Other("el archivo a entregar no existe".into())); }
    let (sha, size, _) = hash_and_head(src).await?;
    if size == 0 { return Err(AppError::Other("el archivo está vacío".into())); }

    let dup: i64 = sqlx::query_scalar("SELECT count(*) FROM submission_version v JOIN submission su ON su.id=v.submission_id WHERE su.activity_id=? AND v.sha256=? AND v.state NOT IN ('error','rejected')")
        .bind(activity_id).bind(&sha).fetch_one(pool).await?;
    if dup > 0 { return Err(AppError::Other("ya existe una versión activa de esta entrega con exactamente el mismo contenido".into())); }

    let name = src.file_name().and_then(|n| n.to_str()).unwrap_or("entrega").to_string();
    let clean = sanitize_filename(&name);
    let ext = Path::new(&clean).extension().and_then(|e| e.to_str()).unwrap_or("").to_string();
    let dir = PathBuf::from(&loc.2).join(section_dir(loc.3)).join("Entregas");
    let (stem, dotext) = (Path::new(&clean).file_stem().and_then(|s| s.to_str()).unwrap_or("entrega").to_string(), if ext.is_empty() { String::new() } else { format!(".{ext}") });
    tokio::fs::create_dir_all(safe_join(files_root, &dir)?).await?;
    let mut rel = None;
    for n in 1..1000 {
        let cand = dir.join(if n == 1 { format!("{stem}{dotext}") } else { format!("{stem}_v{n}{dotext}") });
        let full = safe_join(files_root, &cand)?;
        match tokio::fs::OpenOptions::new().write(true).create_new(true).open(&full).await {
            Ok(_) => { tokio::fs::copy(src, &full).await?; rel = Some(cand); break; }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    let rel = rel.ok_or_else(|| AppError::Other("demasiadas copias con el mismo nombre".into()))?;
    let rel_s = rel.to_string_lossy().to_string();
    let local_name = rel.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();

    let res: AppResult<i64> = async {
        let mut tx = pool.begin().await?;
        let sid: i64 = match sqlx::query_scalar("SELECT id FROM submission WHERE activity_id=?").bind(activity_id).fetch_optional(&mut *tx).await? {
            Some(s) => s,
            None => sqlx::query_scalar("INSERT INTO submission(activity_id,deadline_at,created_at) VALUES(?,?,strftime('%s','now')) RETURNING id").bind(activity_id).bind(loc.4).fetch_one(&mut *tx).await?,
        };
        let fid: i64 = sqlx::query_scalar("INSERT INTO local_file(uuid,original_name,local_name,rel_path,extension,size_bytes,sha256,downloaded_at,sync_state) VALUES(?,?,?,?,?,?,?,strftime('%s','now'),'personal') RETURNING id")
            .bind(uuid::Uuid::new_v4().to_string()).bind(&name).bind(&local_name).bind(&rel_s).bind(&ext).bind(size).bind(&sha).fetch_one(&mut *tx).await?;
        sqlx::query("INSERT INTO file_association(file_id,course_id,section_id,activity_id,role,created_at) VALUES(?,?,?,?, 'work', strftime('%s','now'))").bind(fid).bind(loc.0).bind(loc.1).bind(activity_id).execute(&mut *tx).await?;
        let ver: i64 = sqlx::query_scalar("SELECT coalesce(max(version),0)+1 FROM submission_version WHERE submission_id=?").bind(sid).fetch_one(&mut *tx).await?;
        let vid: i64 = sqlx::query_scalar("INSERT INTO submission_version(submission_id,version,file_id,sha256,state) VALUES(?,?,?,?,'prepared') RETURNING id").bind(sid).bind(ver).bind(fid).bind(&sha).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok(vid)
    }.await;
    match res {
        Ok(vid) => get(pool, vid).await,
        Err(e) => { let _ = tokio::fs::remove_file(files_root.join(&rel)).await; Err(e) }
    }
}

pub async fn set_in_progress(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let n = sqlx::query("UPDATE submission_version SET state='in_progress' WHERE id=? AND state IN ('prepared','in_progress','pending_confirmation')").bind(id).execute(pool).await?.rows_affected();
    if n == 0 { return Err(AppError::Other("esta versión ya no admite el proceso de entrega".into())); }
    Ok(())
}

pub async fn cancel(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let n = sqlx::query("UPDATE submission_version SET state='error', error_detail='Cancelada por el usuario antes de confirmarse' WHERE id=? AND state IN ('prepared','in_progress','pending_confirmation')").bind(id).execute(pool).await?.rows_affected();
    if n == 0 { return Err(AppError::Other("solo se pueden cancelar versiones aún no confirmadas".into())); }
    Ok(())
}

#[derive(Debug, PartialEq)]
pub enum Decision { Confirmed(String), Pending(String) }

/// Decide, sin tocar la red, si lo que muestra Moodle prueba la recepción. `submitted` = (nombre, sha256 del contenido descargado).
pub fn decide(prepared_sha: &str, moodle_status: Option<&str>, submitted: &[(String, String)]) -> Decision {
    let status = moodle_status.unwrap_or("").to_string();
    let low = status.to_lowercase();
    if submitted.is_empty() { return Decision::Pending(format!("Moodle no muestra ningún archivo enviado (estado: «{status}»).")); }
    let hit = submitted.iter().find(|(_, sha)| sha == prepared_sha);
    match hit {
        Some((name, _)) if low.contains("borrador") => Decision::Pending(format!("«{name}» está en Moodle pero como borrador: falta pulsar «Enviar tarea».")),
        Some((name, _)) if low.contains("enviado") => Decision::Confirmed(format!("Moodle indica «{status}» y el archivo «{name}» que muestra tiene el mismo SHA-256 que la copia preparada ({}).", &prepared_sha[..16])),
        Some((name, _)) => Decision::Pending(format!("«{name}» coincide, pero Moodle no indica que esté enviado (estado: «{status}»).")),
        None => Decision::Pending(format!("Moodle muestra otro contenido ({}), distinto al preparado.", submitted.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", "))),
    }
}

pub async fn apply_decision(pool: &SqlitePool, id: i64, d: &Decision, sent_at: Option<i64>, grade: Option<&str>, feedback: Option<&str>) -> AppResult<()> {
    match d {
        Decision::Confirmed(ev) => { let n = sqlx::query("UPDATE submission_version SET state='confirmed', confirmed_at=strftime('%s','now'), sent_at=coalesce(?,sent_at), confirmation_evidence=?, grade=coalesce(?,grade), teacher_feedback=coalesce(?,teacher_feedback), error_detail=NULL WHERE id=? AND state IN ('prepared','in_progress','pending_confirmation')")
            .bind(sent_at).bind(ev).bind(grade).bind(feedback).bind(id).execute(pool).await?.rows_affected();
            if n == 1 {
                let name: Option<String> = sqlx::query_scalar("SELECT a.name FROM submission_version v JOIN submission su ON su.id=v.submission_id JOIN activity a ON a.id=su.activity_id WHERE v.id=?").bind(id).fetch_optional(pool).await?;
                crate::reminders::emit(pool, "submission", 1, &format!("Entrega confirmada: {}", name.unwrap_or_default()), Some("Moodle muestra el archivo con el mismo SHA-256 que la copia preparada."), Some("version"), Some(id)).await;
            } }
        Decision::Pending(ev) => { sqlx::query("UPDATE submission_version SET state='pending_confirmation', confirmation_evidence=? WHERE id=? AND state IN ('prepared','in_progress','pending_confirmation')").bind(ev).bind(id).execute(pool).await?; }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, moodle::models::*, sync};

    // DATOS DE PRUEBA sintéticos: no proceden de Moodle real.
    async fn setup() -> (tempfile::TempDir, SqlitePool, i64) {
        let d = tempfile::tempdir().unwrap();
        let p = db::open(&d.path().join("t.db")).await.unwrap();
        sync::apply_courses(&p, &[RemoteCourse { moodle_id: 1, fullname: "Desarrollo de Interfaces".into(), shortname: None, url: String::new() }]).await.unwrap();
        let a = RemoteActivity { cmid: 10, modname: "assign".into(), name: "Tarea 1".into(), url: None, due_at: None, files: vec![] };
        let r = RemoteActivity { cmid: 11, modname: "resource".into(), name: "Apuntes".into(), url: None, due_at: None, files: vec![] };
        sync::apply_contents(&p, 1, &[RemoteSection { moodle_id: Some(9), position: 3, name: "T3".into(), activities: vec![a, r] }]).await.unwrap();
        let aid: i64 = sqlx::query_scalar("SELECT id FROM activity WHERE moodle_cmid=10").fetch_one(&p).await.unwrap();
        (d, p, aid)
    }
    async fn src(d: &Path, name: &str, data: &[u8]) -> PathBuf { let p = d.join(name); tokio::fs::write(&p, data).await.unwrap(); p }

    #[tokio::test]
    async fn prepare_makes_independent_versioned_copy() {
        let (d, p, aid) = setup().await;
        let root = d.path().join("files");
        let s = src(d.path(), "mi trabajo.pdf", b"%PDF-1 version uno").await;
        let v1 = prepare(&p, &root, aid, &s).await.unwrap();
        assert_eq!((v1.version, v1.state.as_str()), (1, "prepared"));
        assert_eq!(v1.rel_path, "Desarrollo_de_Interfaces/Tema_03/Entregas/mi_trabajo.pdf");
        // el usuario sigue editando el original: la copia histórica no cambia
        tokio::fs::write(&s, b"%PDF-1 EDITADO despues").await.unwrap();
        assert_eq!(tokio::fs::read(root.join(&v1.rel_path)).await.unwrap(), b"%PDF-1 version uno");
        // mismo contenido que una versión activa: rechazado; contenido distinto: versión 2 sin pisar la 1
        assert!(prepare(&p, &root, aid, &src(d.path(), "otra.pdf", b"%PDF-1 version uno").await).await.is_err());
        let v2 = prepare(&p, &root, aid, &s).await.unwrap();
        assert_eq!(v2.version, 2);
        assert_ne!(v1.rel_path, v2.rel_path);
        assert!(root.join(&v1.rel_path).exists());
        assert_eq!(history(&p, Some(aid)).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn only_assignments_and_real_nonempty_files() {
        let (d, p, aid) = setup().await;
        let root = d.path().join("files");
        let res: i64 = sqlx::query_scalar("SELECT id FROM activity WHERE moodle_cmid=11").fetch_one(&p).await.unwrap();
        assert!(prepare(&p, &root, res, &src(d.path(), "a.pdf", b"x").await).await.is_err(), "un recurso no admite entregas");
        assert!(prepare(&p, &root, aid, &src(d.path(), "vacio.txt", b"").await).await.is_err());
        assert!(prepare(&p, &root, aid, &d.path().join("noexiste.pdf")).await.is_err());
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM local_file").fetch_one(&p).await.unwrap();
        assert_eq!(n, 0, "los rechazos no dejan filas ni archivos");
    }

    #[test]
    fn confirmation_requires_matching_hash_and_sent_status() {
        let h = "a".repeat(64);
        let other = "b".repeat(64);
        assert!(matches!(decide(&h, Some("Enviado para calificar"), &[("x.pdf".into(), h.clone())]), Decision::Confirmed(_)));
        // mismo contenido pero borrador: NO confirmada
        assert!(matches!(decide(&h, Some("Borrador (no enviado)"), &[("x.pdf".into(), h.clone())]), Decision::Pending(_)));
        // hay un envío pero es otro archivo: NO confirmada
        assert!(matches!(decide(&h, Some("Enviado para calificar"), &[("x.pdf".into(), other)]), Decision::Pending(_)));
        // sin archivos: NO confirmada
        assert!(matches!(decide(&h, Some("Todavía no se han realizado envíos"), &[]), Decision::Pending(_)));
        // estado desconocido aunque coincida el hash: NO confirmada
        assert!(matches!(decide(&h, None, &[("x.pdf".into(), h.clone())]), Decision::Pending(_)));
    }

    #[tokio::test]
    async fn state_flow_and_immutable_history() {
        let (d, p, aid) = setup().await;
        let root = d.path().join("files");
        let v = prepare(&p, &root, aid, &src(d.path(), "t.pdf", b"%PDF-1 a").await).await.unwrap();
        set_in_progress(&p, v.id).await.unwrap();
        apply_decision(&p, v.id, &Decision::Pending("falta enviar".into()), None, None, None).await.unwrap();
        assert_eq!(get(&p, v.id).await.unwrap().state, "pending_confirmation");
        apply_decision(&p, v.id, &Decision::Confirmed("ok".into()), Some(1234), Some("9,00 / 10,00"), Some("bien")).await.unwrap();
        let c = get(&p, v.id).await.unwrap();
        assert_eq!((c.state.as_str(), c.sent_at, c.grade.as_deref()), ("confirmed", Some(1234), Some("9,00 / 10,00")));
        assert!(c.confirmed_at.is_some());
        // una versión confirmada no se cancela ni se reabre
        assert!(cancel(&p, v.id).await.is_err());
        assert!(set_in_progress(&p, v.id).await.is_err());
        // y el historial no se puede borrar ni alterar el archivo/hash
        assert!(sqlx::query("DELETE FROM submission_version WHERE id=?").bind(v.id).execute(&p).await.is_err());
        assert!(sqlx::query("UPDATE submission_version SET sha256='x' WHERE id=?").bind(v.id).execute(&p).await.is_err());
    }

    #[tokio::test]
    async fn cancelled_version_keeps_its_record() {
        let (d, p, aid) = setup().await;
        let root = d.path().join("files");
        let v = prepare(&p, &root, aid, &src(d.path(), "t.pdf", b"%PDF-1 a").await).await.unwrap();
        cancel(&p, v.id).await.unwrap();
        let c = get(&p, v.id).await.unwrap();
        assert_eq!(c.state, "error"); assert!(c.error_detail.unwrap().contains("Cancelada"));
        assert!(root.join(&c.rel_path).exists(), "la copia se conserva");
        // tras cancelar, el mismo contenido puede volver a prepararse
        assert!(prepare(&p, &root, aid, &src(d.path(), "t2.pdf", b"%PDF-1 a").await).await.is_ok());
    }
}
