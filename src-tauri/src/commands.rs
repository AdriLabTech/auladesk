use crate::{db, error::AppResult, state::AppState};
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct AppInfo { version: &'static str, data_dir: String, db_path: String, session_type: Option<String>, schema_tables: i64 }

/// Ajustes que la interfaz puede escribir. Cualquier clave nueva que guarde el frontend DEBE añadirse aquí (si no, se rechaza).
pub const ALLOWED_SETTINGS: [&str; 15] = [
    "theme", "sidebar_collapsed", "moodle_base_url", "remember_session",
    "theme_id", "theme_accent", "theme_radius", "theme_calm", "welcome_enabled", "tasks_view", "tasks_board_by",
    "autosync_enabled", "autosync_minutes", "close_to_tray", "welcome_config",
];

#[tauri::command]
pub async fn app_info(st: State<'_, AppState>) -> AppResult<AppInfo> {
    let tables: i64 = sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE '\\_%' ESCAPE '\\' AND name NOT LIKE 'sqlite_%'")
        .fetch_one(&st.pool).await?;
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        data_dir: st.data_dir.display().to_string(),
        db_path: st.data_dir.join("database/moodledesk.db").display().to_string(),
        session_type: std::env::var("XDG_SESSION_TYPE").ok(),
        schema_tables: tables,
    })
}

#[tauri::command]
pub async fn get_setting(st: State<'_, AppState>, key: String) -> AppResult<Option<String>> {
    db::get_setting(&st.pool, &key).await
}

#[tauri::command]
pub async fn set_setting(st: State<'_, AppState>, key: String, value: String) -> AppResult<()> {
    if !ALLOWED_SETTINGS.contains(&key.as_str()) {
        return Err(crate::error::AppError::Other(format!("ajuste desconocido: {key}")));
    }
    if key == "remember_session" && value == "0" {
        let _ = tauri::async_runtime::spawn_blocking(crate::session_store::forget).await;
    }
    if key == "close_to_tray" { crate::autosync::CLOSE_TO_TRAY.store(value != "0", std::sync::atomic::Ordering::SeqCst); }
    db::set_setting(&st.pool, &key, &value).await
}

#[tauri::command]
pub async fn backup_database(st: State<'_, AppState>) -> AppResult<String> {
    let name = format!("auladesk-{}.db", chrono::Local::now().format("%Y%m%d-%H%M%S"));
    let dest = st.data_dir.join("exports").join(name);
    db::backup(&st.pool, &dest).await?;
    tracing::info!("copia de seguridad creada");
    Ok(dest.display().to_string())
}

// ---------- Moodle ----------
use crate::moodle::{session::SessionState, window};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

#[tauri::command]
pub async fn moodle_open(app: AppHandle, st: State<'_, AppState>) -> AppResult<()> {
    window::open(&app, &st.data_dir, true).await
}

#[tauri::command]
pub async fn moodle_close(app: AppHandle) -> AppResult<()> {
    if let Some(w) = app.get_webview_window(window::LABEL) { w.hide().map_err(|e| crate::error::AppError::Other(e.to_string()))?; }
    Ok(())
}

#[tauri::command]
pub async fn moodle_navigate(app: AppHandle, path: String) -> AppResult<()> {
    let url = window::base_url().join(&path).map_err(|e| crate::error::AppError::Other(e.to_string()))?;
    if !window::is_allowed(&url) { return Err(crate::error::AppError::UnsafePath(path)); }
    let w = app.get_webview_window(window::LABEL).ok_or_else(|| crate::error::AppError::Other("la ventana de Moodle no está abierta".into()))?;
    w.navigate(url).map_err(|e| crate::error::AppError::Other(e.to_string()))
}

pub const PROBES: [&str; 9] = ["dom", "session", "courses", "contents", "files", "logout", "assignment", "activity_dates", "calendar"];

/// Pide a la página de Moodle que ejecute un extractor conocido (lista blanca, sin JS arbitrario).
#[tauri::command]
pub async fn moodle_probe(app: AppHandle, kind: String) -> AppResult<()> {
    if !PROBES.contains(&kind.as_str()) { return Err(crate::error::AppError::Other(format!("probe no permitido: {kind}"))); }
    let w = app.get_webview_window(window::LABEL).ok_or_else(|| crate::error::AppError::Other("la ventana de Moodle no está abierta".into()))?;
    w.eval(format!("window.__mdRun && window.__mdRun('{kind}')")).map_err(|e| crate::error::AppError::Other(e.to_string()))
}

#[derive(Serialize)]
pub struct Account { pub user_id: Option<i64>, pub name: Option<String> }

/// Última cuenta con la que se inició sesión (la guarda `ingest_page`; se borra al olvidar la sesión).
#[tauri::command]
pub async fn moodle_account(st: State<'_, AppState>) -> AppResult<Account> {
    let get = |k: &'static str| { let pool = st.pool.clone(); async move { db::get_setting(&pool, k).await.ok().flatten().filter(|v| !v.is_empty()) } };
    Ok(Account { user_id: get("moodle_user_id").await.and_then(|v| v.parse().ok()).filter(|id| *id > 0), name: get("moodle_user_name").await })
}

#[tauri::command]
pub async fn moodle_status(st: State<'_, AppState>) -> AppResult<SessionState> {
    Ok(*st.session.lock().unwrap())
}

const MAX_PAYLOAD: usize = 4 * 1024 * 1024;

/// Único comando accesible desde páginas de Moodle. Valida origen, tipo y tamaño; nunca ejecuta nada del payload.
#[tauri::command]
pub async fn ingest_page(app: AppHandle, window: WebviewWindow, st: State<'_, AppState>, kind: String, url: String, payload: serde_json::Value) -> AppResult<()> {
    use crate::error::AppError;
    if window.label() != window::LABEL { return Err(AppError::Other("origen no autorizado".into())); }
    let parsed = tauri::Url::parse(&url).map_err(|e| AppError::Other(e.to_string()))?;
    if !window::is_allowed(&parsed) { return Err(AppError::Other("URL fuera de Moodle".into())); }
    let raw = serde_json::to_vec(&payload).map_err(|e| AppError::Other(e.to_string()))?;
    if raw.len() > MAX_PAYLOAD { return Err(AppError::Other("payload demasiado grande".into())); }
    match kind.as_str() {
        "session" => {
            let uid = payload.get("userId").and_then(|v| v.as_i64()).unwrap_or(0);
            let state = if uid > 0 { SessionState::Active } else { SessionState::LoggedOut };
            let prev = std::mem::replace(&mut *st.session.lock().unwrap(), state);
            let state = if prev == SessionState::Active && state == SessionState::LoggedOut { SessionState::Expired } else { state };
            *st.session.lock().unwrap() = state;
            tracing::info!("sesión Moodle: {:?}", state);
            if state == SessionState::Active {
                save_session(&window, &st).await;
                // Cuenta con la que se ha entrado. Si es otra distinta de la anterior y no llega su nombre, se borra el anterior: nunca se muestra el de otra persona.
                let prev = db::get_setting(&st.pool, "moodle_user_id").await.ok().flatten().unwrap_or_default();
                let name = payload.get("fullName").and_then(|v| v.as_str()).and_then(crate::moodle::session::clean_name);
                let _ = db::set_setting(&st.pool, "moodle_user_id", &uid.to_string()).await;
                match name {
                    Some(n) => { let _ = db::set_setting(&st.pool, "moodle_user_name", &n).await; }
                    None if prev != uid.to_string() => { let _ = db::set_setting(&st.pool, "moodle_user_name", "").await; }
                    None => {}
                }
                let _ = app.emit("moodle-account", true);
            }
            let _ = app.emit("moodle-session", state);
        }
        "dom" => {
            // Datos personales del usuario: solo a cache/ local, nunca a logs.
            let name = format!("probe-dom-{}.json", chrono::Utc::now().timestamp());
            let dest = st.data_dir.join("cache").join(&name);
            let body = serde_json::json!({ "url_path": parsed.path(), "data": payload });
            tokio::fs::write(&dest, serde_json::to_vec_pretty(&body).unwrap()).await?;
            tracing::info!("probe dom guardado: {name} ({} bytes)", raw.len());
        }
        "courses" | "course_contents" | "assignment" | "activity_dates" | "calendar" => {
            let covers: Vec<(i64, String)> = if kind == "courses" {
                payload.get("courses").and_then(|c| c.as_array()).map(|a| a.iter().filter_map(|c| Some((c.get("moodle_id")?.as_i64()?, c.get("cover")?.as_str()?.to_string()))).collect()).unwrap_or_default()
            } else { vec![] };
            let res = apply_extraction(&st.pool, &kind, payload).await;
            if res.is_ok() {
                for (mid, uri) in covers {
                    if let Some(name) = crate::library::save_cover(&st.data_dir, mid, &uri).await {
                        let _ = sqlx::query("UPDATE course SET cover_path=? WHERE moodle_id=?").bind(name).bind(mid).execute(&st.pool).await;
                    }
                }
            }
            finish(&st, res);
        }
        "submit_stage" => {
            // Resultado de una etapa de la entrega automatizada: ok → siguiente etapa; error → se detiene y se explica.
            let ok = payload.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
            let msg = payload.get("message").and_then(|v| v.as_str()).map(|s| s.chars().take(900).collect::<String>()).unwrap_or_else(|| "etapa de entrega fallida".into());
            tracing::info!("entrega: etapa {} → {}", payload.get("stage").and_then(|v| v.as_str()).unwrap_or("?"), if ok { "ok" } else { "detenida" });
            finish(&st, if ok { Ok(SyncStats::default()) } else { Err(msg) });
        }
        "files" => {
            let res = crate::moodle::mapper::files(payload).map_err(|e| e.to_string());
            if let Some(tx) = st.found_files.lock().unwrap().take() { let _ = tx.send(res); }
        }
        "error" => {
            let msg = payload.get("message").and_then(|v| v.as_str()).unwrap_or("?").to_string();
            let ek = payload.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
            tracing::warn!("extractor {ek}: {msg}");
            crate::sync::log_history(&st.pool, "error", &format!("{ek}: {msg}"), Some("v1"), None).await;
            if let Some(tx) = st.found_files.lock().unwrap().take() { let _ = tx.send(Err(msg.clone())); }
            finish(&st, Err(msg));
        }
        other => return Err(AppError::Other(format!("tipo no permitido: {other}"))),
    }
    Ok(())
}

use crate::sync::{self, SyncStats};

fn finish(st: &AppState, res: Result<SyncStats, String>) {
    if let Some(tx) = st.pending.lock().unwrap().take() { let _ = tx.send(res); }
}

async fn apply_extraction(pool: &sqlx::SqlitePool, kind: &str, payload: serde_json::Value) -> Result<SyncStats, String> {
    use crate::moodle::mapper;
    let r = match kind {
        "courses" => match mapper::courses(payload) { Ok(c) => sync::apply_courses(pool, &c).await, Err(e) => Err(e) },
        "assignment" => match mapper::assignment(payload) { Ok(a) => sync::apply_assignment(pool, &a).await, Err(e) => Err(e) },
        "activity_dates" => match mapper::activity_dates(payload) { Ok((cmid, _, o, c)) => sync::apply_activity_dates(pool, cmid, o, c).await, Err(e) => Err(e) },
        "calendar" => match mapper::calendar_events(payload) { Ok(ev) => crate::calendar::apply_page_events(pool, &ev).await.map(|n| SyncStats { seen: ev.len() as i64, changed: n, removed: 0 }), Err(e) => Err(e) },
        _ => match mapper::contents(payload) { Ok((id, s)) => sync::apply_contents(pool, id, &s).await, Err(e) => Err(e) },
    };
    match r {
        Ok(st) => { sync::log_history(pool, "info", &format!("{kind} aplicado"), Some("v1"), Some(&st)).await; Ok(st) }
        Err(e) => { tracing::warn!("{kind} rechazado: {e}"); sync::log_history(pool, "error", &format!("{kind}: {e}"), Some("v1"), None).await; Err(e.to_string()) }
    }
}

// ---------- Orquestación ----------
use std::time::Duration;

pub async fn step_pub(app: &AppHandle, st: &AppState, path: &str, probe: &str) -> Result<SyncStats, String> { step(app, st, path, probe).await }

async fn step(app: &AppHandle, st: &AppState, path: &str, probe: &str) -> Result<SyncStats, String> {
    let w = app.get_webview_window(window::LABEL).ok_or("la ventana de Moodle no está abierta")?;
    let url = window::base_url().join(path).map_err(|e| e.to_string())?;
    if !window::is_allowed(&url) { return Err("ruta fuera de Moodle".into()); }
    let (tx, rx) = tokio::sync::oneshot::channel();
    *st.pending.lock().unwrap() = Some(tx);
    tokio::time::sleep(Duration::from_millis(300)).await; // deja pasar eventos de carga rezagados
    let mut loads = st.page_loads.subscribe();
    loads.borrow_and_update();
    w.navigate(url.clone()).map_err(|e| e.to_string())?;
    crate::library::wait_page(&w, &mut loads, url.path(), false).await?;
    w.eval(format!("window.__mdRun && window.__mdRun('{probe}')")).map_err(|e| e.to_string())?;
    match tokio::time::timeout(Duration::from_secs(40), rx).await {
        Ok(Ok(r)) => r,
        Ok(Err(_)) => Err("extracción cancelada".into()),
        Err(_) => { st.pending.lock().unwrap().take(); Err("el extractor no respondió (¿sesión caducada o estructura cambiada?)".into()) }
    }
}

#[derive(Serialize, Clone)]
pub struct SyncSummary { pub courses: i64, pub synced: i64, pub assignments: i64, pub failed: Vec<String> }

pub async fn run_sync(app: AppHandle) -> Result<SyncSummary, String> {
    use std::sync::atomic::Ordering;
    let st = app.state::<AppState>();
    if *st.session.lock().unwrap() != SessionState::Active { return Err("No hay sesión de Moodle activa. Inicia sesión primero.".into()); }
    if st.sync_running.swap(true, Ordering::SeqCst) { return Err("Ya hay una sincronización en curso.".into()); }
    let r = run_sync_inner(&app, &st).await;
    st.sync_running.store(false, Ordering::SeqCst);
    r
}

async fn run_sync_inner(app: &AppHandle, st: &AppState) -> Result<SyncSummary, String> {
    let job: i64 = sqlx::query_scalar("INSERT INTO sync_job(kind,state,started_at) VALUES('full','running',strftime('%s','now')) RETURNING id")
        .fetch_one(&st.pool).await.map_err(|e| e.to_string())?;
    let finish_job = |state: &'static str| {
        let pool = st.pool.clone();
        async move { let _ = sqlx::query("UPDATE sync_job SET state=?, finished_at=strftime('%s','now') WHERE id=?").bind(state).bind(job).execute(&pool).await; }
    };
    if let Err(e) = step(app, st, "my/courses.php", "courses").await {
        finish_job("failed").await;
        return Err(format!("Lista de cursos: {e}"));
    }
    let courses: Vec<(i64, String)> = sqlx::query_as("SELECT moodle_id, fullname FROM course WHERE removed_at IS NULL ORDER BY fullname")
        .fetch_all(&st.pool).await.map_err(|e| e.to_string())?;
    let total = courses.len() as i64;
    let (mut ok, mut failed) = (0, vec![]);
    for (i, (mid, name)) in courses.iter().enumerate() {
        let _ = app.emit("sync-progress", serde_json::json!({ "done": i, "total": total, "current": name }));
        match step(app, st, &format!("course/view.php?id={mid}"), "contents").await {
            Ok(_) => ok += 1,
            Err(e) => failed.push(format!("{name}: {e}")),
        }
    }
    // Fase 2: estado y fechas de cada tarea (una página por tarea).
    let assigns: Vec<(i64, String)> = sqlx::query_as("SELECT moodle_cmid, name FROM activity WHERE modname='assign' AND removed_at IS NULL ORDER BY course_id, id")
        .fetch_all(&st.pool).await.map_err(|e| e.to_string())?;
    let n = assigns.len() as i64;
    for (i, (cmid, name)) in assigns.iter().enumerate() {
        let _ = app.emit("sync-progress", serde_json::json!({ "done": total + i as i64, "total": total + n, "current": name }));
        if let Err(e) = step(app, st, &format!("mod/assign/view.php?id={cmid}"), "assignment").await { failed.push(format!("Tarea «{name}»: {e}")); }
    }
    // Fase 3: fechas de cuestionarios y calendario de Moodle («Próximos eventos»).
    let quizzes: Vec<(i64, String)> = sqlx::query_as("SELECT moodle_cmid, name FROM activity WHERE modname='quiz' AND removed_at IS NULL ORDER BY id").fetch_all(&st.pool).await.map_err(|e| e.to_string())?;
    for (cmid, name) in &quizzes {
        if let Err(e) = step(app, st, &format!("mod/quiz/view.php?id={cmid}"), "activity_dates").await { failed.push(format!("Cuestionario «{name}»: {e}")); }
    }
    if let Err(e) = step(app, st, "calendar/view.php?view=upcoming", "calendar").await { failed.push(format!("Calendario de Moodle: {e}")); }
    if let Err(e) = crate::calendar::sync_derived(&st.pool).await { failed.push(format!("Eventos derivados: {e}")); }
    let _ = app.emit("sync-progress", serde_json::json!({ "done": total + n, "total": total + n, "current": null }));
    if !failed.is_empty() {
        crate::reminders::emit(&st.pool, "sync_error", 1, "Error de sincronización", Some(&failed.iter().take(3).cloned().collect::<Vec<_>>().join("\n")), None, None).await;
    }
    crate::notifier::tick(&st.pool, &crate::notifier::NotifySend, chrono::Utc::now().timestamp()).await;
    finish_job(if failed.is_empty() { "done" } else { "failed" }).await;
    Ok(SyncSummary { courses: total, synced: ok, assignments: n, failed })
}

#[tauri::command]
pub async fn moodle_sync(app: AppHandle) -> Result<SyncSummary, String> { run_sync(app).await }

#[derive(Serialize, sqlx::FromRow)]
pub struct CourseRow { files: i64, id: i64, moodle_id: i64, fullname: String, url: Option<String>, sections: i64, activities: i64, assignments: i64, last_synced_at: Option<i64>, removed_at: Option<i64> }

#[tauri::command]
pub async fn list_courses(st: State<'_, AppState>) -> AppResult<Vec<CourseRow>> {
    Ok(sqlx::query_as(
        "SELECT c.id, c.moodle_id, c.fullname, c.url, c.last_synced_at, c.removed_at,
           (SELECT count(*) FROM course_section s WHERE s.course_id=c.id AND s.removed_at IS NULL) AS sections,
           (SELECT count(*) FROM activity a WHERE a.course_id=c.id AND a.removed_at IS NULL) AS activities,
           (SELECT count(*) FROM activity a WHERE a.course_id=c.id AND a.removed_at IS NULL AND a.modname='assign') AS assignments,
           (SELECT count(DISTINCT f.file_id) FROM file_association f WHERE f.course_id=c.id) AS files
         FROM course c WHERE c.removed_at IS NULL ORDER BY c.fullname").fetch_all(&st.pool).await?)
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ActivityRow { files: i64, id: i64, section_position: i64, section_name: String, modname: String, name: String, url: Option<String> }

#[tauri::command]
pub async fn course_tree(st: State<'_, AppState>, course_id: i64) -> AppResult<Vec<ActivityRow>> {
    Ok(sqlx::query_as(
        "SELECT a.id, s.position AS section_position, s.name AS section_name, a.modname, a.name, a.url,
           (SELECT count(*) FROM file_association f WHERE f.activity_id=a.id) AS files
         FROM activity a JOIN course_section s ON s.id=a.section_id
         WHERE a.course_id=? AND a.removed_at IS NULL AND s.removed_at IS NULL ORDER BY s.position, a.id").bind(course_id).fetch_all(&st.pool).await?)
}

/// Guarda en el llavero las cookies del WebView (solo dominios de la Junta) si el usuario no lo ha desactivado.
async fn save_session(window: &WebviewWindow, st: &AppState) {
    if db::get_setting(&st.pool, "remember_session").await.ok().flatten().as_deref() == Some("0") { return; }
    let Ok(all) = window.cookies() else { return };
    let keep: Vec<_> = all.iter().filter(|c| crate::session_store::relevant(c.domain())).map(crate::session_store::to_stored).collect();
    if keep.is_empty() { return; }
    match tauri::async_runtime::spawn_blocking(move || crate::session_store::save(&keep)).await {
        Ok(Ok(())) => tracing::info!("sesión guardada en el llavero"),
        Ok(Err(e)) => tracing::warn!("no se pudo guardar la sesión en el llavero: {e}"),
        Err(e) => tracing::warn!("guardar sesión: {e}"),
    }
}

/// Cierra la sesión de Moodle (enlace oficial de logout con sesskey) y borra la sesión guardada y las cookies locales.
#[tauri::command]
pub async fn moodle_forget_session(app: AppHandle, st: State<'_, AppState>) -> AppResult<()> {
    use crate::error::AppError;
    if let Some(w) = app.get_webview_window(window::LABEL) {
        let _ = w.eval("window.__mdRun && window.__mdRun('logout')");
        tokio::time::sleep(Duration::from_millis(1500)).await;
        if let Ok(cookies) = w.cookies() { for c in cookies.into_iter().filter(|c| crate::session_store::relevant(c.domain())) { let _ = w.delete_cookie(c); } }
    }
    tauri::async_runtime::spawn_blocking(crate::session_store::forget).await.map_err(|e| AppError::Other(e.to_string()))?.map_err(AppError::Other)?;
    *st.session.lock().unwrap() = SessionState::LoggedOut;
    for k in ["moodle_user_id", "moodle_user_name"] { let _ = db::set_setting(&st.pool, k, "").await; }
    let _ = app.emit("moodle-account", true);
    let _ = app.emit("moodle-session", SessionState::LoggedOut);
    tracing::info!("sesión olvidada");
    Ok(())
}

#[cfg(test)]
mod settings_tests {
    use super::ALLOWED_SETTINGS;
    /// Claves de apariencia y vistas que el frontend persiste (lib/theme.svelte.ts, views/Tasks.svelte). Si falla, el ajuste no se guardaría.
    #[test]
    fn frontend_setting_keys_are_allowed() {
        for k in ["theme_id", "theme_accent", "theme_radius", "theme_calm", "welcome_enabled", "tasks_view", "tasks_board_by", "sidebar_collapsed", "remember_session", "autosync_enabled", "autosync_minutes", "close_to_tray", "welcome_config"] {
            assert!(ALLOWED_SETTINGS.contains(&k), "{k} no está permitido");
        }
        // las claves de cuenta las escribe solo el backend: la interfaz no debe poder falsearlas
        assert!(!ALLOWED_SETTINGS.contains(&"moodle_user_name") && !ALLOWED_SETTINGS.contains(&"moodle_user_id"));
    }
}
