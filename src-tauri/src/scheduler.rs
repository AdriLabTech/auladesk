//! Entregas programadas. El usuario confirma UNA vez (tarea, archivo, SHA-256 y hora) y a esa hora la app intenta entregar
//! con el mismo camino verificado de `submission_submit`. Si algo no cuadra, se detiene y avisa; nunca insiste sin límite.
//! Límites reales: necesita la app abierta (o en la bandeja) y una sesión de Moodle viva (caduca a las 8 h de inactividad).
use crate::error::{AppError, AppResult};
use crate::moodle::session::SessionState;
use crate::state::AppState;
use serde::Serialize;
use sqlx::SqlitePool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const MAX_ATTEMPTS: i64 = 3;
pub const RETRY_SECS: i64 = 600;
const MIN_LEAD_SECS: i64 = 30;

#[derive(Debug, Serialize, sqlx::FromRow, Clone)]
pub struct ScheduleRow {
    pub id: i64, pub version_id: i64, pub run_at: i64, pub state: String, pub attempts: i64,
    pub last_attempt_at: Option<i64>, pub last_error: Option<String>, pub replace_existing: i64, pub seen: i64, pub created_at: i64,
    pub activity_name: String, pub course_name: String, pub file_name: String, pub sha256: String, pub deadline_at: Option<i64>,
}

const SELECT: &str = "SELECT s.id, s.version_id, s.run_at, s.state, s.attempts, s.last_attempt_at, s.last_error, s.replace_existing, s.seen, s.created_at,
    a.name AS activity_name, c.fullname AS course_name, f.original_name AS file_name, v.sha256, su.deadline_at
    FROM submission_schedule s JOIN submission_version v ON v.id=s.version_id JOIN submission su ON su.id=v.submission_id
    JOIN activity a ON a.id=su.activity_id JOIN course c ON c.id=a.course_id JOIN local_file f ON f.id=v.file_id";

/// Qué hacer con una entrega programada en este instante. Función pura para poder probar cada regla.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Action { Wait, Run, Missed, GiveUp, Blocked }

pub struct Facts { pub now: i64, pub run_at: i64, pub deadline_at: Option<i64>, pub attempts: i64, pub last_attempt_at: Option<i64>, pub session_active: bool, pub busy: bool }

pub fn decide(f: &Facts) -> Action {
    if f.now < f.run_at { return Action::Wait; }
    if f.deadline_at.is_some_and(|d| f.now > d) { return Action::Missed; }          // el plazo ya pasó: no se intenta
    if f.attempts >= MAX_ATTEMPTS { return Action::GiveUp; }
    if f.last_attempt_at.is_some_and(|t| f.now < t + RETRY_SECS) { return Action::Wait; }
    if !f.session_active || f.busy { return Action::Blocked; }
    Action::Run
}

/// Valida y registra la programación. `confirmed` debe venir de la confirmación explícita (diálogo con tarea, archivo, hash y hora).
pub async fn create(pool: &SqlitePool, files_root: &std::path::Path, version_id: i64, run_at: i64, replace: bool, confirmed: bool, now: i64) -> AppResult<i64> {
    if !confirmed { return Err(AppError::Other("Falta la confirmación explícita del usuario.".into())); }
    let v = crate::submissions::get(pool, version_id).await?;
    if !matches!(v.state.as_str(), "prepared" | "in_progress" | "pending_confirmation") { return Err(AppError::Other(format!("Esta versión está en estado «{}» y no se puede programar.", v.state))); }
    if run_at < now + MIN_LEAD_SECS { return Err(AppError::Other("La hora programada debe estar en el futuro.".into())); }
    if let Some(d) = v.deadline_at { if run_at > d { return Err(AppError::Other("La hora programada es posterior al cierre de la tarea.".into())); } }
    let path = crate::paths::safe_join(files_root, std::path::Path::new(&v.rel_path))?;
    let bytes = tokio::fs::read(&path).await.map_err(|_| AppError::Other("El archivo preparado ya no está en disco.".into()))?;
    if crate::files::sha256_bytes(&bytes) != v.sha256 { return Err(AppError::Other("El archivo preparado ha cambiado en disco; prepáralo de nuevo.".into())); }
    let id = sqlx::query_scalar("INSERT INTO submission_schedule(version_id,run_at,confirmed_sha256,replace_existing,created_at,updated_at) VALUES(?,?,?,?,?,?) RETURNING id")
        .bind(version_id).bind(run_at).bind(&v.sha256).bind(replace as i64).bind(now).bind(now).fetch_one(pool).await
        .map_err(|e| if e.to_string().contains("UNIQUE") { AppError::Other("Esta versión ya tiene una entrega programada. Cancélala antes de cambiarla.".into()) } else { e.into() })?;
    Ok(id)
}

pub async fn list(pool: &SqlitePool, only_active: bool) -> AppResult<Vec<ScheduleRow>> {
    let w = if only_active { "WHERE s.state IN ('scheduled','running')" } else { "" };
    Ok(sqlx::query_as(&format!("{SELECT} {w} ORDER BY s.run_at DESC LIMIT 200")).fetch_all(pool).await?)
}

pub async fn cancel(pool: &SqlitePool, id: i64, now: i64) -> AppResult<()> {
    let n = sqlx::query("UPDATE submission_schedule SET state='cancelled', updated_at=?, seen=1 WHERE id=? AND state='scheduled'").bind(now).bind(id).execute(pool).await?.rows_affected();
    if n == 0 { return Err(AppError::Other("Solo se puede cancelar una entrega programada que aún no se ha iniciado.".into())); }
    Ok(())
}

async fn set_state(pool: &SqlitePool, id: i64, state: &str, err: Option<&str>, now: i64) {
    let _ = sqlx::query("UPDATE submission_schedule SET state=?, last_error=COALESCE(?,last_error), updated_at=? WHERE id=?").bind(state).bind(err).bind(now).bind(id).execute(pool).await;
}

async fn notify(pool: &SqlitePool, prio: i64, title: &str, body: &str, version_id: i64) {
    let _ = crate::notif::add(pool, "entrega", prio, title, Some(body), Some("submission_version"), Some(version_id)).await;
}

/// Un ciclo: revisa las programadas vencidas y ejecuta como mucho una.
pub async fn tick(app: &AppHandle) {
    let st = app.state::<AppState>();
    let now = chrono::Utc::now().timestamp();
    let due: Vec<ScheduleRow> = sqlx::query_as(&format!("{SELECT} WHERE s.state='scheduled' AND s.run_at<=? ORDER BY s.run_at LIMIT 10")).bind(now).fetch_all(&st.pool).await.unwrap_or_default();
    for s in due {
        // Si la versión ya no es entregable (cancelada o confirmada por otra vía) o cambió de contenido, la programación se anula.
        let (vstate, vsha): (String, String) = match sqlx::query_as("SELECT state, sha256 FROM submission_version WHERE id=?").bind(s.version_id).fetch_one(&st.pool).await { Ok(x) => x, Err(_) => continue };
        let confirmed_sha: String = sqlx::query_scalar("SELECT confirmed_sha256 FROM submission_schedule WHERE id=?").bind(s.id).fetch_one(&st.pool).await.unwrap_or_default();
        if !matches!(vstate.as_str(), "prepared" | "in_progress" | "pending_confirmation") || vsha != confirmed_sha { set_state(&st.pool, s.id, "cancelled", Some("La versión ya no es entregable o cambió."), now).await; continue; }
        let facts = Facts { now, run_at: s.run_at, deadline_at: s.deadline_at, attempts: s.attempts, last_attempt_at: s.last_attempt_at,
            session_active: *st.session.lock().unwrap() == SessionState::Active, busy: st.sync_running.load(Ordering::SeqCst) };
        match decide(&facts) {
            Action::Wait => {}
            Action::Missed => { set_state(&st.pool, s.id, "missed", Some("El plazo cerró antes de poder entregar."), now).await;
                notify(&st.pool, 2, "Entrega programada NO realizada", &format!("{} ({}): el plazo cerró antes de poder entregar.", s.activity_name, s.course_name), s.version_id).await; }
            Action::GiveUp => { set_state(&st.pool, s.id, "failed", None, now).await;
                notify(&st.pool, 2, "Entrega programada fallida", &format!("{} ({}): {MAX_ATTEMPTS} intentos sin éxito. {}", s.activity_name, s.course_name, s.last_error.clone().unwrap_or_default()), s.version_id).await; }
            Action::Blocked => {
                // Solo se avisa la primera vez que se bloquea; los reintentos esperan a que vuelva la sesión.
                if s.last_error.is_none() {
                    let why = if facts.busy { "hay otra operación con Moodle en curso" } else { "no hay sesión de Moodle activa: abre AulaDesk e inicia sesión" };
                    sqlx::query("UPDATE submission_schedule SET last_error=?, updated_at=? WHERE id=?").bind(why).bind(now).bind(s.id).execute(&st.pool).await.ok();
                    if !facts.busy { notify(&st.pool, 2, "Entrega programada pendiente", &format!("{} ({}): {why}. Se reintentará hasta el cierre.", s.activity_name, s.course_name), s.version_id).await; }
                }
            }
            Action::Run => { run_one(app, &s, now).await; break; }
        }
    }
}

async fn run_one(app: &AppHandle, s: &ScheduleRow, now: i64) {
    let st = app.state::<AppState>();
    sqlx::query("UPDATE submission_schedule SET state='running', attempts=attempts+1, last_attempt_at=?, updated_at=? WHERE id=?").bind(now).bind(now).bind(s.id).execute(&st.pool).await.ok();
    let _ = app.emit("schedule-update", s.id);
    tracing::info!("entrega programada {}: iniciando (versión {})", s.id, s.version_id);
    let end = chrono::Utc::now().timestamp();
    match crate::submission_cmds::submit_confirmed(app, s.version_id, s.replace_existing != 0).await {
        Ok(v) if v.state == "confirmed" => {
            set_state(&st.pool, s.id, "done", Some(""), end).await;
            notify(&st.pool, 1, "Entrega programada confirmada", &format!("{} ({}): Moodle muestra el archivo idéntico.", s.activity_name, s.course_name), s.version_id).await;
        }
        Ok(_) => {
            // Enviada pero Moodle aún no muestra el archivo idéntico: no se repite la subida (podría duplicar); queda para comprobar a mano.
            set_state(&st.pool, s.id, "done", Some("Enviada; pendiente de comprobar."), end).await;
            notify(&st.pool, 1, "Entrega programada enviada", &format!("{} ({}): enviada, pero Moodle aún no confirma el archivo. Pulsa «Comprobar» en Entregas.", s.activity_name, s.course_name), s.version_id).await;
        }
        Err(e) => {
            // Vuelve a 'scheduled' para reintentar (límite de intentos y de plazo en `decide`).
            sqlx::query("UPDATE submission_schedule SET state='scheduled', last_error=?, updated_at=? WHERE id=?").bind(&e).bind(end).bind(s.id).execute(&st.pool).await.ok();
            notify(&st.pool, 2, "Entrega programada detenida", &format!("{} ({}): {e}", s.activity_name, s.course_name), s.version_id).await;
        }
    }
    let _ = app.emit("schedule-update", s.id);
}

pub async fn run_loop(app: AppHandle) {
    // Al arrancar, una programación que quedó 'running' (la app se cerró a mitad) se devuelve a 'scheduled': submit_confirmed comprueba antes de actuar.
    { let st = app.state::<AppState>(); let _ = sqlx::query("UPDATE submission_schedule SET state='scheduled', last_error='La app se cerró durante el intento.' WHERE state='running'").execute(&st.pool).await; }
    tokio::time::sleep(Duration::from_secs(15)).await;
    loop { tick(&app).await; tokio::time::sleep(Duration::from_secs(20)).await; }
}

// ───────────── Aviso emergente al abrir ─────────────
#[derive(Serialize)]
pub struct PendingItem { pub version: crate::submissions::VersionRow, pub schedule: Option<ScheduleRow>, pub can_submit: Option<i64> }

#[derive(Serialize)]
pub struct PendingPrompt { pub items: Vec<PendingItem>, pub results: Vec<ScheduleRow> }

/// Lo que se muestra al entrar: entregas preparadas (con o sin programación) y resultados de programaciones no vistos.
pub async fn pending_prompt(pool: &SqlitePool) -> AppResult<PendingPrompt> {
    let versions: Vec<crate::submissions::VersionRow> = crate::submissions::history(pool, None).await?
        .into_iter().filter(|v| matches!(v.state.as_str(), "prepared" | "in_progress" | "pending_confirmation")).collect();
    let mut items = Vec::new();
    for v in versions {
        let schedule: Option<ScheduleRow> = sqlx::query_as(&format!("{SELECT} WHERE s.version_id=? AND s.state IN ('scheduled','running') LIMIT 1")).bind(v.id).fetch_optional(pool).await?;
        let can_submit: Option<i64> = sqlx::query_scalar("SELECT can_submit FROM assignment WHERE activity_id=?").bind(v.activity_id).fetch_optional(pool).await?.flatten();
        items.push(PendingItem { version: v, schedule, can_submit });
    }
    items.sort_by_key(|i| i.version.deadline_at.unwrap_or(i64::MAX));
    let results = sqlx::query_as(&format!("{SELECT} WHERE s.seen=0 AND s.state IN ('done','failed','missed') ORDER BY s.updated_at DESC LIMIT 20")).fetch_all(pool).await?;
    Ok(PendingPrompt { items, results })
}

// ───────────── Comandos ─────────────
#[tauri::command]
pub async fn schedule_create(st: tauri::State<'_, AppState>, version_id: i64, run_at: i64, replace: Option<bool>, confirmed: bool) -> AppResult<i64> {
    create(&st.pool, &st.data_dir.join("files"), version_id, run_at, replace.unwrap_or(false), confirmed, chrono::Utc::now().timestamp()).await
}
#[tauri::command]
pub async fn schedule_list(st: tauri::State<'_, AppState>, only_active: Option<bool>) -> AppResult<Vec<ScheduleRow>> { list(&st.pool, only_active.unwrap_or(false)).await }
#[tauri::command]
pub async fn schedule_cancel(st: tauri::State<'_, AppState>, id: i64) -> AppResult<()> { cancel(&st.pool, id, chrono::Utc::now().timestamp()).await }
#[tauri::command]
pub async fn pending_prompt_data(st: tauri::State<'_, AppState>) -> AppResult<PendingPrompt> { pending_prompt(&st.pool).await }
#[tauri::command]
pub async fn schedule_mark_seen(st: tauri::State<'_, AppState>) -> AppResult<()> {
    sqlx::query("UPDATE submission_schedule SET seen=1 WHERE state IN ('done','failed','missed','cancelled')").execute(&st.pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn f() -> Facts { Facts { now: 1000, run_at: 900, deadline_at: Some(5000), attempts: 0, last_attempt_at: None, session_active: true, busy: false } }
    #[test]
    fn waits_until_run_time() { assert_eq!(decide(&Facts { now: 899, ..f() }), Action::Wait); assert_eq!(decide(&f()), Action::Run); }
    #[test]
    fn missed_after_deadline_even_with_session() { assert_eq!(decide(&Facts { now: 5001, ..f() }), Action::Missed); assert_eq!(decide(&Facts { now: 5000, ..f() }), Action::Run); }
    #[test]
    fn no_deadline_still_runs() { assert_eq!(decide(&Facts { deadline_at: None, ..f() }), Action::Run); }
    #[test]
    fn blocked_without_session_or_when_busy() {
        assert_eq!(decide(&Facts { session_active: false, ..f() }), Action::Blocked);
        assert_eq!(decide(&Facts { busy: true, ..f() }), Action::Blocked);
    }
    #[test]
    fn retries_are_spaced_and_capped() {
        assert_eq!(decide(&Facts { attempts: 1, last_attempt_at: Some(1000 - RETRY_SECS + 1), ..f() }), Action::Wait);
        assert_eq!(decide(&Facts { attempts: 1, last_attempt_at: Some(1000 - RETRY_SECS), ..f() }), Action::Run);
        assert_eq!(decide(&Facts { attempts: MAX_ATTEMPTS, ..f() }), Action::GiveUp);
    }

    // ── Programación contra una BD real con datos sintéticos ──
    use crate::{db, moodle::models::*, sync};
    async fn setup() -> (tempfile::TempDir, SqlitePool, i64) {
        let d = tempfile::tempdir().unwrap();
        let p = db::open(&d.path().join("t.db")).await.unwrap();
        sync::apply_courses(&p, &[RemoteCourse { moodle_id: 1, fullname: "Interfaces".into(), shortname: None, url: String::new() }]).await.unwrap();
        let a = RemoteActivity { cmid: 10, modname: "assign".into(), name: "Tarea 1".into(), url: None, due_at: None, files: vec![] };
        sync::apply_contents(&p, 1, &[RemoteSection { moodle_id: Some(9), position: 3, name: "T3".into(), activities: vec![a] }]).await.unwrap();
        let aid: i64 = sqlx::query_scalar("SELECT id FROM activity WHERE moodle_cmid=10").fetch_one(&p).await.unwrap();
        let src = d.path().join("t.pdf"); tokio::fs::write(&src, b"%PDF-1 a").await.unwrap();
        let v = crate::submissions::prepare(&p, &d.path().join("files"), aid, &src).await.unwrap();
        (d, p, v.id)
    }

    #[tokio::test]
    async fn create_requires_confirmation_future_time_and_deadline() {
        let (d, p, vid) = setup().await;
        let root = d.path().join("files");
        assert!(create(&p, &root, vid, 10_000, false, false, 1000).await.is_err(), "sin confirmación explícita");
        assert!(create(&p, &root, vid, 1010, false, true, 1000).await.is_err(), "hora ya pasada o casi");
        sqlx::query("UPDATE submission SET deadline_at=5000").execute(&p).await.unwrap();
        assert!(create(&p, &root, vid, 6000, false, true, 1000).await.is_err(), "después del cierre");
        let id = create(&p, &root, vid, 4000, false, true, 1000).await.unwrap();
        assert!(create(&p, &root, vid, 4100, false, true, 1000).await.is_err(), "una sola programación activa por versión");
        let all = list(&p, true).await.unwrap();
        assert_eq!((all.len(), all[0].state.as_str(), all[0].run_at), (1, "scheduled", 4000));
        cancel(&p, id, 1001).await.unwrap();
        assert!(cancel(&p, id, 1002).await.is_err(), "ya cancelada");
        assert!(create(&p, &root, vid, 4200, false, true, 1003).await.is_ok(), "tras cancelar se puede reprogramar");
    }

    #[tokio::test]
    async fn create_rejects_file_changed_on_disk() {
        let (d, p, vid) = setup().await;
        let root = d.path().join("files");
        let v = crate::submissions::get(&p, vid).await.unwrap();
        tokio::fs::write(root.join(&v.rel_path), b"manipulado").await.unwrap();
        assert!(create(&p, &root, vid, 9000, false, true, 1000).await.unwrap_err().to_string().contains("cambiado"));
    }

    #[tokio::test]
    async fn prompt_lists_prepared_and_unseen_results() {
        let (d, p, vid) = setup().await;
        let root = d.path().join("files");
        let pr = pending_prompt(&p).await.unwrap();
        assert_eq!((pr.items.len(), pr.items[0].schedule.is_none(), pr.results.len()), (1, true, 0));
        let id = create(&p, &root, vid, 9000, false, true, 1000).await.unwrap();
        assert!(pending_prompt(&p).await.unwrap().items[0].schedule.is_some());
        set_state(&p, id, "missed", Some("x"), 2000).await;
        let pr = pending_prompt(&p).await.unwrap();
        assert_eq!(pr.results.len(), 1, "resultado sin ver");
        sqlx::query("UPDATE submission_schedule SET seen=1").execute(&p).await.unwrap();
        assert_eq!(pending_prompt(&p).await.unwrap().results.len(), 0);
        // una versión confirmada deja de ser «preparada»
        crate::submissions::apply_decision(&p, vid, &crate::submissions::Decision::Confirmed("ok".into()), Some(1), None, None).await.unwrap();
        assert!(pending_prompt(&p).await.unwrap().items.is_empty());
    }
}
