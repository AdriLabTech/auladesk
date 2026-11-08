//! Comandos de entregas. «Entregar» nunca se automatiza: AulaDesk abre la página de Moodle y el usuario envía.
use crate::commands::step_pub;
use crate::error::AppResult;
use crate::library::download_hash;
use crate::moodle::{mapper::SubmittedFile, session::SessionState, window};
use crate::state::AppState;
use crate::submissions::{self, VersionRow};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn submission_prepare(st: State<'_, AppState>, activity_id: i64, path: String) -> AppResult<VersionRow> {
    submissions::prepare(&st.pool, &st.data_dir.join("files"), activity_id, &PathBuf::from(path)).await
}

#[tauri::command]
pub async fn submission_history(st: State<'_, AppState>, activity_id: Option<i64>) -> AppResult<Vec<VersionRow>> {
    submissions::history(&st.pool, activity_id).await
}

#[tauri::command]
pub async fn submission_cancel(st: State<'_, AppState>, id: i64) -> AppResult<()> { submissions::cancel(&st.pool, id).await }

/// Abre en Moodle la página oficial de edición de la entrega. `confirmed` debe venir de una confirmación explícita del usuario.
#[tauri::command]
pub async fn submission_start(app: AppHandle, id: i64, confirmed: bool) -> Result<(), String> {
    if !confirmed { return Err("Falta la confirmación explícita del usuario.".into()); }
    let st = app.state::<AppState>();
    if *st.session.lock().unwrap() != SessionState::Active { return Err("No hay sesión de Moodle activa.".into()); }
    let v = submissions::get(&st.pool, id).await.map_err(|e| e.to_string())?;
    let (cmid, can): (i64, Option<i64>) = sqlx::query_as("SELECT a.moodle_cmid, x.can_submit FROM activity a LEFT JOIN assignment x ON x.activity_id=a.id WHERE a.id=?")
        .bind(v.activity_id).fetch_one(&st.pool).await.map_err(|e| e.to_string())?;
    if can == Some(0) { return Err("Moodle no ofrece entregar esta tarea ahora (plazo cerrado o sin permiso). Sincroniza para actualizar su estado.".into()); }
    submissions::set_in_progress(&st.pool, id).await.map_err(|e| e.to_string())?;
    window::open(&app, &st.data_dir, true).await.map_err(|e| e.to_string())?;
    let w = app.get_webview_window(window::LABEL).ok_or("La ventana de Moodle no está abierta.")?;
    let url = window::base_url().join(&format!("mod/assign/view.php?id={cmid}&action=editsubmission")).map_err(|e| e.to_string())?;
    w.navigate(url).map_err(|e| e.to_string())?;
    tracing::info!("proceso de entrega abierto para la versión {id}");
    Ok(())
}

/// Lee la página de la tarea, descarga lo que Moodle muestra como enviado y compara hashes. Solo así puede quedar «confirmed».
#[tauri::command]
pub async fn submission_verify(app: AppHandle, id: i64) -> Result<VersionRow, String> {
    let st = app.state::<AppState>();
    if *st.session.lock().unwrap() != SessionState::Active { return Err("No hay sesión de Moodle activa.".into()); }
    if st.sync_running.swap(true, Ordering::SeqCst) { return Err("Hay otra operación con Moodle en curso.".into()); }
    let r = verify_inner(&app, &st, id).await;
    st.sync_running.store(false, Ordering::SeqCst);
    r
}

async fn verify_inner(app: &AppHandle, st: &AppState, id: i64) -> Result<VersionRow, String> {
    let v = submissions::get(&st.pool, id).await.map_err(|e| e.to_string())?;
    if !matches!(v.state.as_str(), "prepared" | "in_progress" | "pending_confirmation") { return Err(format!("Esta versión está en estado «{}» y no se puede comprobar.", v.state)); }
    let cmid: i64 = sqlx::query_scalar("SELECT moodle_cmid FROM activity WHERE id=?").bind(v.activity_id).fetch_one(&st.pool).await.map_err(|e| e.to_string())?;
    step_pub(app, st, &format!("mod/assign/view.php?id={cmid}"), "assignment").await?;
    let (status, files_json, modified, grade, feedback): (Option<String>, Option<String>, Option<i64>, Option<String>, Option<String>) =
        sqlx::query_as("SELECT submission_status, submitted_files_json, last_modified_at, grade, feedback FROM assignment WHERE activity_id=?").bind(v.activity_id).fetch_one(&st.pool).await.map_err(|e| e.to_string())?;
    let files: Vec<SubmittedFile> = files_json.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default();
    let mut hashes = Vec::new();
    for f in files.iter().take(10) { hashes.push((f.name.clone(), download_hash(app, st, &f.url).await?.0)); }
    let d = submissions::decide(&v.sha256, status.as_deref(), &hashes);
    submissions::apply_decision(&st.pool, id, &d, modified, grade.as_deref(), feedback.as_deref()).await.map_err(|e| e.to_string())?;
    crate::sync::log_history(&st.pool, "info", &format!("comprobación de entrega v{} → {}", v.version, if matches!(d, submissions::Decision::Confirmed(_)) { "confirmada" } else { "pendiente" }), Some("v1"), None).await;
    submissions::get(&st.pool, id).await.map_err(|e| e.to_string())
}
