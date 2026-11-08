//! Comandos de calendario, notificaciones y servicio en segundo plano.
use crate::calendar::{self, EventRow, NewEvent};
use crate::error::{AppError, AppResult};
use crate::notif::{self, NotifRow};
use crate::notifier::{self, NotifySend, ServiceStatus, Sink};
use crate::reminders::{self, NotifSettings};
use crate::state::AppState;
use tauri::State;

fn now() -> i64 { chrono::Utc::now().timestamp() }

async fn rearm(st: &AppState) { let s = reminders::load_settings(&st.pool).await; let _ = reminders::reconcile(&st.pool, now(), &s).await; }

#[tauri::command]
pub async fn calendar_list(st: State<'_, AppState>, from: i64, to: i64) -> AppResult<Vec<EventRow>> { calendar::list(&st.pool, from, to).await }

#[tauri::command]
pub async fn calendar_create(st: State<'_, AppState>, event: NewEvent) -> AppResult<i64> { let id = calendar::create_personal(&st.pool, &event).await?; rearm(&st).await; Ok(id) }

#[tauri::command]
pub async fn calendar_update(st: State<'_, AppState>, id: i64, event: NewEvent) -> AppResult<()> { calendar::update_personal(&st.pool, id, &event).await?; rearm(&st).await; Ok(()) }

#[tauri::command]
pub async fn calendar_delete(st: State<'_, AppState>, id: i64) -> AppResult<()> { calendar::delete_personal(&st.pool, id).await }

#[tauri::command]
pub async fn calendar_set_notes(st: State<'_, AppState>, id: i64, notes: String) -> AppResult<()> { calendar::set_notes(&st.pool, id, &notes).await }

#[tauri::command]
pub async fn notif_list(st: State<'_, AppState>, limit: i64, unread_only: bool) -> AppResult<Vec<NotifRow>> { notif::list(&st.pool, limit, unread_only).await }

#[tauri::command]
pub async fn notif_mark_read(st: State<'_, AppState>, id: Option<i64>) -> AppResult<()> { notif::mark_read(&st.pool, id).await }

#[tauri::command]
pub async fn notif_unread_count(st: State<'_, AppState>) -> AppResult<i64> { notif::unread_count(&st.pool).await }

#[tauri::command]
pub async fn notif_settings_get(st: State<'_, AppState>) -> AppResult<NotifSettings> { Ok(reminders::load_settings(&st.pool).await) }

#[tauri::command]
pub async fn notif_settings_set(st: State<'_, AppState>, settings: NotifSettings) -> AppResult<NotifSettings> {
    let s = reminders::save_settings(&st.pool, &settings).await?;
    rearm(&st).await;
    Ok(s)
}

/// Envía una notificación de prueba al escritorio (ignora el horario silencioso: es una prueba explícita).
#[tauri::command]
pub async fn notif_test(st: State<'_, AppState>) -> AppResult<()> {
    let id = notif::add(&st.pool, "reminder", 0, "Prueba de AulaDesk", Some("Si ves esto, las notificaciones de escritorio funcionan."), None, None).await?;
    let n: NotifRow = sqlx::query_as("SELECT id,category,priority,title,body,ref_kind,ref_id,created_at,read_at,shown_at FROM notification WHERE id=?").bind(id).fetch_one(&st.pool).await?;
    NotifySend.show(&n).map_err(AppError::Other)?;
    sqlx::query("UPDATE notification SET shown_at=? WHERE id=?").bind(now()).bind(id).execute(&st.pool).await?;
    Ok(())
}

#[tauri::command]
pub async fn service_status() -> AppResult<ServiceStatus> { tauri::async_runtime::spawn_blocking(notifier::service_status).await.map_err(|e| AppError::Other(e.to_string())) }

#[tauri::command]
pub async fn service_install() -> AppResult<ServiceStatus> { tauri::async_runtime::spawn_blocking(notifier::service_install).await.map_err(|e| AppError::Other(e.to_string()))?.map_err(AppError::Other) }

#[tauri::command]
pub async fn service_uninstall() -> AppResult<ServiceStatus> { tauri::async_runtime::spawn_blocking(notifier::service_uninstall).await.map_err(|e| AppError::Other(e.to_string()))?.map_err(AppError::Other) }
