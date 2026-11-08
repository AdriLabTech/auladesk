//! Sincronización automática periódica y bandeja del sistema.
//! La sesión de Moodle vive en el WebView oculto, así que solo se sincroniza con la app en marcha (ventana abierta u oculta en la bandeja).
use crate::moodle::session::SessionState;
use crate::{commands, state::AppState};
use serde::Serialize;
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// La bandeja se creó con éxito (si falta la librería del sistema, queda en false y cerrar la ventana cierra la app).
pub static TRAY_OK: AtomicBool = AtomicBool::new(false);
/// Ajuste «cerrar a la bandeja» (se refresca al guardarlo, para poder leerlo desde el manejador de eventos de ventana).
pub static CLOSE_TO_TRAY: AtomicBool = AtomicBool::new(true);

pub const MIN_MINUTES: i64 = 10;
pub const MAX_MINUTES: i64 = 720;
pub const DEFAULT_MINUTES: i64 = 30;
pub fn clamp_minutes(m: i64) -> i64 { m.clamp(MIN_MINUTES, MAX_MINUTES) }

/// `last` = (fin de la última sincronización, si fue correcta). Tras un fallo se reintenta antes (mitad del intervalo, mínimo 5 min).
pub fn next_at(last: Option<(i64, bool)>, minutes: i64) -> Option<i64> {
    let every = clamp_minutes(minutes) * 60;
    last.map(|(at, ok)| at + if ok { every } else { (every / 2).max(300) })
}
pub fn is_due(last: Option<(i64, bool)>, now: i64, minutes: i64) -> bool { next_at(last, minutes).map_or(true, |n| now >= n) }

async fn config(pool: &SqlitePool) -> (bool, i64) {
    let enabled = crate::db::get_setting(pool, "autosync_enabled").await.ok().flatten().as_deref() != Some("0");
    let minutes = crate::db::get_setting(pool, "autosync_minutes").await.ok().flatten().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_MINUTES);
    (enabled, clamp_minutes(minutes))
}

async fn last_job(pool: &SqlitePool) -> Option<(i64, bool)> {
    sqlx::query_as::<_, (i64, String)>("SELECT finished_at, state FROM sync_job WHERE kind='full' AND finished_at IS NOT NULL ORDER BY id DESC LIMIT 1")
        .fetch_optional(pool).await.ok().flatten().map(|(t, s)| (t, s == "done"))
}

#[derive(Serialize)]
pub struct AutoSyncStatus { pub enabled: bool, pub minutes: i64, pub last_at: Option<i64>, pub last_ok: Option<bool>, pub next_at: Option<i64>, pub running: bool, pub session_active: bool, pub tray: bool, pub close_to_tray: bool }

#[tauri::command]
pub async fn autosync_status(st: tauri::State<'_, AppState>) -> Result<AutoSyncStatus, String> {
    let (enabled, minutes) = config(&st.pool).await;
    let last = last_job(&st.pool).await;
    Ok(AutoSyncStatus {
        enabled, minutes, last_at: last.map(|l| l.0), last_ok: last.map(|l| l.1), next_at: if enabled { next_at(last, minutes) } else { None },
        running: st.sync_running.load(Ordering::SeqCst), session_active: *st.session.lock().unwrap() == SessionState::Active,
        tray: TRAY_OK.load(Ordering::SeqCst), close_to_tray: CLOSE_TO_TRAY.load(Ordering::SeqCst),
    })
}

/// Bucle: cada 30 s comprueba si toca. No sincroniza sin sesión activa ni si ya hay una sincronización en curso.
pub async fn run_loop(app: AppHandle) {
    tokio::time::sleep(Duration::from_secs(20)).await;
    loop {
        let st = app.state::<AppState>();
        let (enabled, minutes) = config(&st.pool).await;
        if enabled && *st.session.lock().unwrap() == SessionState::Active && !st.sync_running.load(Ordering::SeqCst)
            && is_due(last_job(&st.pool).await, chrono::Utc::now().timestamp(), minutes)
        {
            tracing::info!("sincronización automática");
            let _ = app.emit("autosync", "running");
            if let Err(e) = commands::run_sync(app.clone()).await { tracing::warn!("sincronización automática: {e}"); }
            let _ = app.emit("autosync", "idle");
        }
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") { let _ = w.show(); let _ = w.unminimize(); let _ = w.set_focus(); }
}

/// Crea el icono de bandeja. Si la librería del sistema (libayatana-appindicator) no está, la creación puede entrar en pánico: se captura y la app sigue sin bandeja.
pub fn build_tray(app: &AppHandle) {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let app2 = app.clone();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || -> tauri::Result<()> {
        let open = MenuItem::with_id(&app2, "open", "Abrir AulaDesk", true, None::<&str>)?;
        let sync = MenuItem::with_id(&app2, "sync", "Sincronizar ahora", true, None::<&str>)?;
        let quit = MenuItem::with_id(&app2, "quit", "Salir", true, None::<&str>)?;
        let menu = Menu::with_items(&app2, &[&open, &sync, &quit])?;
        let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?;
        TrayIconBuilder::with_id("main").icon(icon).tooltip("AulaDesk").menu(&menu)
            .on_menu_event(|app, ev| match ev.id.as_ref() {
                "open" => show_main(app),
                "sync" => { let a = app.clone(); tauri::async_runtime::spawn(async move { let _ = commands::run_sync(a).await; }); }
                "quit" => app.exit(0),
                _ => {}
            })
            .on_tray_icon_event(|tray, ev| { if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev { show_main(tray.app_handle()); } })
            .build(&app2)?;
        Ok(())
    }));
    match r {
        Ok(Ok(())) => { TRAY_OK.store(true, Ordering::SeqCst); tracing::info!("bandeja del sistema activa"); }
        Ok(Err(e)) => tracing::warn!("no se pudo crear la bandeja: {e}"),
        Err(_) => tracing::warn!("bandeja del sistema no disponible (¿falta libayatana-appindicator?)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn due_rules() {
        assert!(is_due(None, 1000, 30), "nunca sincronizado: toca");
        assert!(!is_due(Some((1000, true)), 1000 + 29 * 60, 30));
        assert!(is_due(Some((1000, true)), 1000 + 30 * 60, 30));
        // tras un fallo se reintenta a la mitad (mínimo 5 min)
        assert!(!is_due(Some((1000, false)), 1000 + 14 * 60, 30));
        assert!(is_due(Some((1000, false)), 1000 + 15 * 60, 30));
        assert!(is_due(Some((1000, false)), 1000 + 300, 10), "con 10 min el reintento es de 5");
    }
    #[test]
    fn interval_is_clamped() {
        assert_eq!(clamp_minutes(1), 10);
        assert_eq!(clamp_minutes(100000), 720);
        assert_eq!(next_at(Some((0, true)), 1), Some(600));
        assert_eq!(next_at(None, 30), None);
    }
}
