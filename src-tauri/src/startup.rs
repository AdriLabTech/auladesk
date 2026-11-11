//! Sincronización al iniciar la aplicación (opcional, por plataforma: `sync_start_moodle`, `sync_start_familias`).
//!
//! Al arrancar, si hay una sesión guardada para la plataforma, se espera a que se restaure (hasta 45 s) y se sincroniza en ese momento,
//! Moodle y Familias A LA VEZ. Mientras dura, la interfaz muestra la pantalla de espera (evento `startup-sync` con lo que falta).
//! Sin sesión guardada la plataforma se omite al instante: no se retiene el arranque esperando algo que no va a llegar.
use crate::{commands, db, familias, moodle::session::SessionState, state::AppState};
use std::{sync::Mutex, time::Duration};
use tauri::{AppHandle, Emitter, Manager};

/// Trabajos pendientes del arranque (nombres para mostrar). Vacío = nada en curso.
static PENDING: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
const WAIT_SESSION: Duration = Duration::from_secs(45);

#[tauri::command]
pub fn startup_sync_status() -> Vec<String> { PENDING.lock().unwrap().iter().map(|s| s.to_string()).collect() }

fn publish(app: &AppHandle) { let _ = app.emit("startup-sync", startup_sync_status()); }
fn finish(app: &AppHandle, job: &'static str) { PENDING.lock().unwrap().retain(|j| *j != job); publish(app); }

async fn enabled(pool: &sqlx::SqlitePool, key: &str) -> bool { db::get_setting(pool, key).await.ok().flatten().as_deref() == Some("1") }

/// ¿Hay sesión guardada que restaurar (y el usuario no ha desactivado recordarla)?
async fn has_saved(pool: &sqlx::SqlitePool, remember_key: &str, user: &'static str) -> bool {
    db::get_setting(pool, remember_key).await.ok().flatten().as_deref() != Some("0")
        && tauri::async_runtime::spawn_blocking(move || crate::session_store::load_for(user).is_some()).await.unwrap_or(false)
}

async fn wait_active(app: &AppHandle, familias: bool) -> bool {
    let t0 = std::time::Instant::now();
    loop {
        let st = app.state::<AppState>();
        let s = if familias { *st.familias_session.lock().unwrap() } else { *st.session.lock().unwrap() };
        if s == SessionState::Active { return true; }
        if s == SessionState::Expired || s == SessionState::LoggedOut && t0.elapsed() > Duration::from_secs(8) || t0.elapsed() > WAIT_SESSION { return false; }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
}

async fn moodle(app: AppHandle) {
    if wait_active(&app, false).await {
        loop {
            match commands::run_sync(app.clone()).await {
                Ok(r) => { tracing::info!("sincronización al iniciar (Moodle): {} cursos, {} fallos", r.courses, r.failed.len()); break; }
                // Otra sincronización ya en marcha (p. ej. la automática): se espera a que termine y se da por buena.
                Err(e) if e.contains("en curso") => { tokio::time::sleep(Duration::from_secs(2)).await; if !app.state::<AppState>().sync_running.load(std::sync::atomic::Ordering::SeqCst) { break; } }
                Err(e) => { tracing::warn!("sincronización al iniciar (Moodle): {e}"); break; }
            }
        }
    } else { tracing::info!("sincronización al iniciar (Moodle): sin sesión activa, se omite"); }
    finish(&app, "Moodle");
}

async fn familias(app: AppHandle) {
    if wait_active(&app, true).await {
        match familias::commands::run_sync(app.clone(), true).await {
            Ok(r) => tracing::info!("sincronización al iniciar (Familias): {} elementos, {} fallos", r.seen, r.failed.len()),
            Err(e) => tracing::info!("sincronización al iniciar (Familias): {e}"),
        }
    } else { tracing::info!("sincronización al iniciar (Familias): sin sesión activa, se omite"); }
    app.state::<AppState>().startup_claims_familias.store(false, std::sync::atomic::Ordering::SeqCst);
    finish(&app, "Familias");
}

/// Se lanza una vez al arrancar.
pub async fn run(app: AppHandle) {
    let st = app.state::<AppState>();
    let m = enabled(&st.pool, "sync_start_moodle").await && has_saved(&st.pool, "remember_session", crate::session_store::MOODLE_USER).await;
    let f = enabled(&st.pool, "sync_start_familias").await && has_saved(&st.pool, "remember_session_familias", crate::session_store::FAMILIAS_USER).await;
    if !m && !f { return; }
    if f { st.startup_claims_familias.store(true, std::sync::atomic::Ordering::SeqCst); } // la sincronización «al entrar» de Familias no se duplica
    { let mut p = PENDING.lock().unwrap(); if m { p.push("Moodle"); } if f { p.push("Familias"); } }
    publish(&app);
    let (a1, a2) = (app.clone(), app.clone());
    let jm = async move { if m { moodle(a1).await; } };
    let jf = async move { if f { familias(a2).await; } };
    crate::integration::futures_join_all([Box::pin(jm) as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>, Box::pin(jf)].into_iter()).await;
    tracing::info!("sincronización al iniciar terminada");
}
