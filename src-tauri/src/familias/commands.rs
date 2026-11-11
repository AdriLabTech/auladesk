//! Comandos de Familias y orquestación de la sincronización. El único comando accesible desde páginas remotas es `ingest_familias`.
use super::{mapper, recipes::{self, Recipe, Step}, sync, window};
use crate::accounts::{self, Account, Student};
use crate::error::{AppError, AppResult};
use crate::moodle::session::SessionState;
use crate::state::AppState;
use crate::sync::SyncStats;
use serde::Serialize;
use sqlx::SqlitePool;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State, Webview};

const PLATFORM: &str = "familias";
const MAX_PAYLOAD: usize = 4 * 1024 * 1024;
/// Límites de cortesía con el portal (también documentados en docs/LEGAL.md): una sincronización automática como máximo cada 60 min,
/// una manual como máximo cada 5 min, sin paralelismo y con una pausa entre pasos.
pub const AUTO_MIN_INTERVAL_SECS: i64 = 3600;
pub const MANUAL_COOLDOWN_SECS: i64 = 300;
/// Pausa tras abrir una página por URL. Los clics y desplegables ya no la usan: esperan a un evento real (carga o contenido nuevo).
const STEP_PAUSE_MS: u64 = 150;

/// ¿Se puede sincronizar ahora? `last_attempt` = fin del último intento (correcto o no).
pub fn sync_allowed(now: i64, last_attempt: Option<i64>, manual: bool) -> Result<(), String> {
    let gap = if manual { MANUAL_COOLDOWN_SECS } else { AUTO_MIN_INTERVAL_SECS };
    match last_attempt { Some(t) if now - t < gap => Err(format!("Para no cargar el portal, espera {} min antes de volver a sincronizar.", (gap - (now - t) + 59) / 60)), _ => Ok(()) }
}

async fn account_id(pool: &SqlitePool) -> AppResult<i64> { accounts::ensure(pool, PLATFORM).await }

fn finish(st: &AppState, res: Result<SyncStats, String>) {
    if let Some(tx) = st.familias_pending.lock().unwrap().take() { let _ = tx.send(res); }
}

// ───────────────────────── IPC remoto ─────────────────────────

/// Único comando accesible desde el portal. Valida origen, tamaño y tipo; nunca ejecuta nada del payload.
#[tauri::command]
pub async fn ingest_familias(app: AppHandle, window: Webview, st: State<'_, AppState>, kind: String, url: String, payload: serde_json::Value) -> AppResult<()> {
    if window.label() != window::LABEL { return Err(AppError::Other("origen no autorizado".into())); }
    let parsed = tauri::Url::parse(&url).map_err(|e| AppError::Other(e.to_string()))?;
    if !window::is_portal(&parsed) { return Err(AppError::Other("URL fuera del portal".into())); }
    if serde_json::to_vec(&payload).map_err(|e| AppError::Other(e.to_string()))?.len() > MAX_PAYLOAD { return Err(AppError::Other("payload demasiado grande".into())); }
    let acc = account_id(&st.pool).await?;
    match kind.as_str() {
        "session" => session_update(&app, &st, &window, acc, &payload).await,
        "recon" => {
            let name = format!("familias-recon-{}.json", chrono::Utc::now().timestamp());
            let dest = st.data_dir.join("cache").join(&name);
            tokio::fs::write(&dest, serde_json::to_vec_pretty(&payload).unwrap()).await?;   // esqueleto sin texto personal, solo en local
            tracing::info!("reconocimiento de Familias guardado: {name}");
            finish(&st, Ok(SyncStats::default()));
        }
        "step" => finish(&st, Ok(SyncStats::default())),
        "students" | "grades" | "attendance" | "messages" | "events" => {
            let res = apply(&st.pool, acc, &kind, payload).await;
            finish(&st, res);
        }
        "error" => {
            let msg = payload.get("message").and_then(|v| v.as_str()).map(|m| mapper::clean(m, 300, false)).unwrap_or_else(|| "?".into());
            let ek = payload.get("kind").and_then(|v| v.as_str()).map(|m| mapper::clean(m, 40, false)).unwrap_or_else(|| "?".into());
            tracing::warn!("extractor de Familias {ek}: {msg}");
            finish(&st, Err(msg));
        }
        other => return Err(AppError::Other(format!("tipo no permitido: {other}"))),
    }
    Ok(())
}

async fn session_update(app: &AppHandle, st: &AppState, window: &Webview, acc: i64, payload: &serde_json::Value) {
    let path_kind = payload.get("path_kind").and_then(|v| v.as_str()).unwrap_or("other");
    let has_pw = payload.get("has_password_field").and_then(|v| v.as_bool()).unwrap_or(false);
    // Criterio (reconocimiento 2026-11-09, SIN verificar tras un login real): tras entrar las rutas son /seneca/nav/<token>; el formulario de acceso tiene campo de clave.
    let error_page = payload.get("error_page").and_then(|v| v.as_bool()).unwrap_or(false);
    let prev = *st.familias_session.lock().unwrap();
    // Página de error del portal sin haber estado dentro: la sesión guardada ya no vale en el servidor.
    if error_page && path_kind != "nav" && prev != SessionState::Active {
        if prev != SessionState::Expired { stale_saved_session(app, st, window, acc).await; }
        return;
    }
    let observed = if path_kind == "nav" && !has_pw { Some(SessionState::Active) } else if has_pw || path_kind == "portal" { Some(SessionState::LoggedOut) } else { None };
    let Some(mut state) = observed else { return };
    if prev == SessionState::Active && state == SessionState::LoggedOut { state = SessionState::Expired; }
    if prev == SessionState::Expired && state == SessionState::LoggedOut { return; } // «caducada» se mantiene hasta que vuelvas a entrar
    *st.familias_session.lock().unwrap() = state;
    if prev != state {
        tracing::info!("sesión Familias: {:?}", state);
        match state {
            SessionState::Active => {
                save_cookies(window).await; let _ = accounts::set_state(&st.pool, acc, "connected", None).await;
                // Sincronización inmediata al entrar (o al restaurarse la sesión), en paralelo con la de Moodle; respeta los límites de frecuencia.
                let a = app.clone();
                if !st.startup_claims_familias.load(std::sync::atomic::Ordering::SeqCst) {
                tauri::async_runtime::spawn(async move { tokio::time::sleep(Duration::from_secs(3)).await; if let Err(e) = run_sync(a, false).await { tracing::info!("sincronización al entrar (Familias): {e}"); } });
                }
            }
            SessionState::Expired => { let _ = accounts::set_state(&st.pool, acc, "expired", Some("La sesión del portal ha caducado.")).await; }
            _ => {}
        }
        let _ = app.emit("familias-session", state);
    }
}

/// La sesión restaurada del llavero ya no la acepta el servidor (caducó por inactividad): se descarta, se deja el WebView limpio en el
/// acceso público y se marca «caducada» para que la interfaz pida entrar de nuevo (sin dejar el estado en «sin comprobar»).
async fn stale_saved_session(app: &AppHandle, st: &AppState, window: &Webview, acc: i64) {
    tracing::info!("sesión Familias: la sesión guardada ya no es válida en el servidor; se descarta");
    let _ = tauri::async_runtime::spawn_blocking(|| crate::session_store::forget_for(crate::session_store::FAMILIAS_USER)).await;
    if let Ok(all) = window.cookies() { for c in all.into_iter().filter(|c| crate::session_store::relevant(c.domain())) { let _ = window.delete_cookie(c); } }
    *st.familias_session.lock().unwrap() = SessionState::Expired;
    let _ = accounts::set_state(&st.pool, acc, "expired", Some("La sesión guardada ha caducado en el portal: inicia sesión de nuevo.")).await;
    let _ = window.navigate(window::entry_url());
    let _ = app.emit("familias-session", SessionState::Expired);
}

/// Mientras la sesión está activa: la mantiene viva en el servidor (petición ligera cada 4 min, sin sincronización en curso) y renueva
/// la copia del llavero cada 5 min, por si el portal cambia las cookies. Si la app se cierra, esa copia es la que se restaura.
pub async fn keepalive_loop(app: AppHandle) {
    let (mut last_ping, mut last_save) = (std::time::Instant::now(), std::time::Instant::now());
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;
        let st = app.state::<AppState>();
        if *st.familias_session.lock().unwrap() != SessionState::Active { continue; }
        let Some(w) = window::get(&app) else { continue };
        if last_ping.elapsed() >= Duration::from_secs(240) && !st.familias_running.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = w.eval("window.__fmPing && window.__fmPing()");
            last_ping = std::time::Instant::now();
            tracing::info!("Familias: sesión mantenida activa");
        }
        if last_save.elapsed() >= Duration::from_secs(300) { save_cookies(&w).await; last_save = std::time::Instant::now(); }
    }
}

async fn save_cookies(window: &Webview) {
    let Ok(all) = window.cookies() else { return };
    let keep: Vec<_> = all.iter().filter(|c| crate::session_store::relevant(c.domain())).map(crate::session_store::to_stored).collect();
    if keep.is_empty() { return; }
    match tauri::async_runtime::spawn_blocking(move || crate::session_store::save_for(crate::session_store::FAMILIAS_USER, &keep)).await {
        Ok(Ok(())) => tracing::info!("sesión de Familias guardada en el llavero"),
        Ok(Err(e)) => tracing::warn!("no se pudo guardar la sesión de Familias: {e}"),
        Err(e) => tracing::warn!("guardar sesión de Familias: {e}"),
    }
}

/// Valida con el contrato y persiste. Un rechazo no toca la BD.
pub async fn apply(pool: &SqlitePool, acc: i64, kind: &str, payload: serde_json::Value) -> Result<SyncStats, String> {
    let r = match kind {
        "students" => match mapper::students(payload) { Ok(s) => sync::apply_students(pool, acc, &s).await, Err(e) => Err(e) },
        "grades" => match mapper::grades(payload) { Ok((r, sc, g)) => sync::apply_grades(pool, acc, &r, sc.as_deref(), &g).await, Err(e) => Err(e) },
        "attendance" => match mapper::attendance(payload) { Ok((r, a)) => sync::apply_attendance(pool, acc, &r, &a).await, Err(e) => Err(e) },
        "messages" => match mapper::messages(payload) { Ok((r, k, m)) => sync::apply_messages(pool, acc, r.as_deref(), &k, &m).await, Err(e) => Err(e) },
        "events" => match mapper::events(payload) { Ok((r, e)) => sync::apply_events(pool, acc, r.as_deref(), &e).await, Err(e) => Err(e) },
        _ => Err(AppError::Other("tipo desconocido".into())),
    };
    r.map_err(|e| { tracing::warn!("Familias {kind} rechazado: {e}"); e.to_string() })
}

// ───────────────────────── Orquestación ─────────────────────────

pub fn load_recipes(data_dir: &std::path::Path) -> AppResult<Vec<Recipe>> {
    // Un fichero local permite iterar las recetas sin recompilar; pasa por la misma validación cerrada.
    let local = data_dir.join("familias_recipes.json");
    match std::fs::read_to_string(&local) { Ok(j) => recipes::parse(&j), Err(_) => recipes::parse(recipes::BUNDLED) }
}

async fn wait_load(st: &AppState) {
    let mut loads = st.familias_page_loads.subscribe();
    loads.borrow_and_update();
    let _ = tokio::time::timeout(Duration::from_secs(12), loads.changed()).await;
}

async fn run_extractor(w: &Webview, st: &AppState, js_kind: &str, args: serde_json::Value) -> Result<SyncStats, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    *st.familias_pending.lock().unwrap() = Some(tx);
    let a = serde_json::to_string(&args.to_string()).map_err(|e| e.to_string())?;
    w.eval(format!("window.__fmRun && window.__fmRun('{js_kind}', {a})")).map_err(|e| e.to_string())?;
    match tokio::time::timeout(Duration::from_secs(40), rx).await {
        Ok(Ok(r)) => r,
        Ok(Err(_)) => Err("extracción cancelada".into()),
        Err(_) => { st.familias_pending.lock().unwrap().take(); Err("el portal no respondió (¿sesión caducada o estructura cambiada?)".into()) }
    }
}

struct StepLog(&'static str, std::time::Instant);
impl Drop for StepLog { fn drop(&mut self) { tracing::info!("Familias paso {}: {} ms", self.0, self.1.elapsed().as_millis()); } }

async fn exec_steps(app: &AppHandle, st: &AppState, steps: &[Step], student: Option<&Student>) -> Result<SyncStats, String> {
    let w = window::get(app).ok_or("la ventana de Familias no está abierta")?;
    let mut total = SyncStats::default();
    for step in steps {
        let t_step = std::time::Instant::now();
        let name = match step { Step::Goto(_) => "goto", Step::Click(_) => "click", Step::ClickStudent(_) => "click_student", Step::Select { .. } => "select", Step::WaitMs(_) => "wait", Step::Extract { .. } => "extract" };
        let _guard = StepLog(name, t_step);
        match step {
            Step::Goto(path) => {
                let url = window::entry_url().join(path).map_err(|e| e.to_string())?;
                if !window::is_portal(&url) { return Err("ruta fuera del portal".into()); }
                tokio::time::sleep(Duration::from_millis(300)).await;
                let mut loads = st.familias_page_loads.subscribe(); loads.borrow_and_update();
                w.navigate(url).map_err(|e| e.to_string())?;
                let _ = tokio::time::timeout(Duration::from_secs(15), loads.changed()).await;
                tokio::time::sleep(Duration::from_millis(STEP_PAUSE_MS)).await;
            }
            Step::Click(text) => {
                // Se suscribe ANTES del clic: una carga rápida no se pierde (antes se esperaban 12 s en balde).
                let mut loads = st.familias_page_loads.subscribe(); loads.borrow_and_update();
                run_extractor(&w, st, "click", serde_json::json!({ "text": text })).await?;
                let _ = tokio::time::timeout(Duration::from_secs(12), loads.changed()).await;
            }
            Step::ClickStudent(_) => {
                let s = student.ok_or("receta por alumno sin alumno")?;
                run_extractor(&w, st, "click_student", serde_json::json!({ "name": s.display_name })).await?;
                tokio::time::sleep(Duration::from_millis(STEP_PAUSE_MS)).await;
            }
            // El propio puente espera a que el portal sustituya el desplegable (sin recarga de página); no hay evento de carga que esperar.
            Step::Select { id, option } => { run_extractor(&w, st, "select", serde_json::json!({ "id": id, "option": option })).await?; }
            Step::WaitMs(ms) => tokio::time::sleep(Duration::from_millis(*ms)).await,
            Step::Extract { extractor, kind } => {
                let r = run_extractor(&w, st, &format!("extract:{extractor}"), serde_json::json!({ "kind": kind })).await?;
                total.seen += r.seen; total.changed += r.changed; total.removed += r.removed;
            }
        }
    }
    Ok(total)
}

#[derive(Serialize, Clone, Default)]
pub struct FamiliasSummary { pub recipes: usize, pub seen: i64, pub changed: i64, pub failed: Vec<String> }

async fn log(pool: &SqlitePool, acc: i64, ok: bool, kind: &str, st: &SyncStats, msg: Option<&str>) {
    let _ = sqlx::query("INSERT INTO integration_sync_history(account_id,at,ok,kind,items_seen,items_changed,message) VALUES(?,?,?,?,?,?,?)")
        .bind(acc).bind(chrono::Utc::now().timestamp()).bind(ok as i64).bind(kind).bind(st.seen).bind(st.changed).bind(msg.map(|m| m.chars().take(400).collect::<String>())).execute(pool).await;
}

async fn last_attempt(pool: &SqlitePool, acc: i64) -> Option<i64> {
    sqlx::query_scalar("SELECT max(at) FROM integration_sync_history WHERE account_id=? AND kind='full'").bind(acc).fetch_one(pool).await.ok().flatten()
}

async fn paused(pool: &SqlitePool) -> bool { crate::db::get_setting(pool, "familias_paused").await.ok().flatten().as_deref() == Some("1") }

pub async fn run_sync(app: AppHandle, manual: bool) -> Result<FamiliasSummary, String> {
    use std::sync::atomic::Ordering;
    let st = app.state::<AppState>();
    let acc = account_id(&st.pool).await.map_err(|e| e.to_string())?;
    if *st.familias_session.lock().unwrap() != SessionState::Active { return Err("No hay sesión de Familias activa. Inicia sesión primero.".into()); }
    if paused(&st.pool).await { return Err("La sincronización de Familias está en pausa.".into()); }
    sync_allowed(chrono::Utc::now().timestamp(), last_attempt(&st.pool, acc).await, manual)?;
    if st.familias_running.swap(true, Ordering::SeqCst) { return Err("Ya hay una sincronización de Familias en curso.".into()); }
    let r = run_sync_inner(&app, &st, acc).await;
    st.familias_running.store(false, Ordering::SeqCst);
    r
}

async fn run_sync_inner(app: &AppHandle, st: &AppState, acc: i64) -> Result<FamiliasSummary, String> {
    let recs = load_recipes(&st.data_dir).map_err(|e| e.to_string())?;
    if recs.is_empty() {
        let msg = "Todavía no hay recetas de navegación para el portal: hace falta el reconocimiento con la cuenta de la familia.";
        log(&st.pool, acc, false, "full", &SyncStats::default(), Some(msg)).await;
        return Err(msg.into());
    }
    let mut sum = FamiliasSummary { recipes: recs.len(), ..Default::default() };
    for r in &recs {
        let targets: Vec<Option<Student>> = if r.per_student {
            accounts::students(&st.pool, acc).await.map_err(|e| e.to_string())?.into_iter().map(Some).collect()
        } else { vec![None] };
        for s in targets {
            match exec_steps(app, st, &r.steps, s.as_ref()).await {
                Ok(t) => { sum.seen += t.seen; sum.changed += t.changed; }
                Err(e) => sum.failed.push(format!("{}{}: {e}", r.name, s.as_ref().map(|s| format!(" ({})", s.display_name)).unwrap_or_default())),
            }
        }
    }
    let ok = sum.failed.is_empty();
    log(&st.pool, acc, ok, "full", &SyncStats { seen: sum.seen, changed: sum.changed, removed: 0 }, sum.failed.first().map(|s| s.as_str())).await;
    if ok { let _ = accounts::set_state(&st.pool, acc, "connected", None).await; }
    else { let _ = accounts::set_state(&st.pool, acc, "connected", sum.failed.first().map(|s| s.as_str())).await; }
    let _ = app.emit("familias-synced", ());
    Ok(sum)
}

/// Bucle de sincronización automática: comprueba cada 60 s y respeta el intervalo mínimo de `sync_allowed`.
pub async fn run_loop(app: AppHandle) {
    tokio::time::sleep(Duration::from_secs(20)).await;
    loop {
        let st = app.state::<AppState>();
        if *st.familias_session.lock().unwrap() == SessionState::Active && !paused(&st.pool).await && !st.familias_running.load(std::sync::atomic::Ordering::SeqCst) {
            if let Ok(acc) = account_id(&st.pool).await {
                if sync_allowed(chrono::Utc::now().timestamp(), last_attempt(&st.pool, acc).await, false).is_ok() {
                    if let Err(e) = run_sync(app.clone(), false).await { tracing::warn!("sincronización automática de Familias: {e}"); }
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

// ───────────────────────── Comandos de la interfaz ─────────────────────────

#[derive(Serialize)]
pub struct FamiliasStatus {
    pub session: SessionState, pub account: Account, pub students: Vec<Student>, pub active_student: Option<i64>, pub paused: bool,
    pub last_sync_at: Option<i64>, pub last_error: Option<String>, pub recipes: usize, pub window_open: bool,
}

#[tauri::command]
pub async fn familias_open(app: AppHandle, st: State<'_, AppState>) -> AppResult<()> { window::open(&app, &st.data_dir, true).await }

#[tauri::command]
pub async fn familias_close(app: AppHandle) -> AppResult<()> { window::set_visible(&app, false); Ok(()) }

#[tauri::command]
pub async fn familias_status(app: AppHandle, st: State<'_, AppState>) -> AppResult<FamiliasStatus> {
    let acc = account_id(&st.pool).await?;
    let account = accounts::list(&st.pool).await?.into_iter().find(|a| a.id == acc).ok_or_else(|| AppError::Other("cuenta no encontrada".into()))?;
    let students = accounts::students(&st.pool, acc).await?;
    let active = active_student(&st.pool, acc).await;
    let last: Option<(i64, Option<String>)> = sqlx::query_as("SELECT at, message FROM integration_sync_history WHERE account_id=? AND kind='full' ORDER BY id DESC LIMIT 1").bind(acc).fetch_optional(&st.pool).await?;
    let last_ok: Option<i64> = sqlx::query_scalar("SELECT max(at) FROM integration_sync_history WHERE account_id=? AND kind='full' AND ok=1").bind(acc).fetch_one(&st.pool).await?;
    let session = *st.familias_session.lock().unwrap();
    let is_paused = paused(&st.pool).await;
    Ok(FamiliasStatus {
        session, account, students, active_student: active, paused: is_paused,
        last_sync_at: last_ok, last_error: last.and_then(|l| l.1), recipes: load_recipes(&st.data_dir).map(|r| r.len()).unwrap_or(0), window_open: window::get(&app).is_some(),
    })
}

async fn active_student(pool: &SqlitePool, acc: i64) -> Option<i64> {
    let id: i64 = crate::db::get_setting(pool, "familias_active_student").await.ok().flatten()?.parse().ok()?;
    accounts::check_student(pool, acc, id).await.ok().map(|_| id)
}

/// El alumno activo lo elige el usuario; cada consulta de datos recibe el id EXPLÍCITO y se valida contra la cuenta.
#[tauri::command]
pub async fn familias_set_active_student(st: State<'_, AppState>, student_id: i64) -> AppResult<()> {
    let acc = account_id(&st.pool).await?;
    accounts::check_student(&st.pool, acc, student_id).await?;
    crate::db::set_setting(&st.pool, "familias_active_student", &student_id.to_string()).await
}

#[tauri::command]
pub async fn familias_sync(app: AppHandle) -> Result<FamiliasSummary, String> { run_sync(app, true).await }

/// Pide al portal un esqueleto estructural de la página actual (sin texto personal) para escribir los extractores.
#[tauri::command]
pub async fn familias_recon(app: AppHandle) -> AppResult<()> {
    let w = window::get(&app).ok_or_else(|| AppError::Other("la ventana de Familias no está abierta".into()))?;
    w.eval("window.__fmRun && window.__fmRun('recon')").map_err(|e| AppError::Other(e.to_string()))
}

#[tauri::command]
pub async fn familias_pause(st: State<'_, AppState>, paused: bool) -> AppResult<()> { crate::db::set_setting(&st.pool, "familias_paused", if paused { "1" } else { "0" }).await }

/// Desvincular: cierra y olvida la sesión, borra cookies del WebView y del llavero y TODOS los datos de la cuenta.
#[tauri::command]
pub async fn familias_disconnect(app: AppHandle, st: State<'_, AppState>) -> AppResult<()> {
    let acc = account_id(&st.pool).await?;
    if let Some(w) = window::get(&app) {
        if let Ok(cookies) = w.cookies() { for c in cookies.into_iter().filter(|c| crate::session_store::relevant(c.domain())) { let _ = w.delete_cookie(c); } }
        let _ = w.navigate(window::entry_url());
    }
    tauri::async_runtime::spawn_blocking(|| crate::session_store::forget_for(crate::session_store::FAMILIAS_USER)).await.map_err(|e| AppError::Other(e.to_string()))?.map_err(AppError::Other)?;
    accounts::purge_data(&st.pool, acc).await?;
    for k in ["familias_active_student"] { let _ = crate::db::set_setting(&st.pool, k, "").await; }
    *st.familias_session.lock().unwrap() = SessionState::LoggedOut;
    let _ = app.emit("familias-session", SessionState::LoggedOut);
    tracing::info!("cuenta de Familias desvinculada");
    Ok(())
}

// ── Datos (siempre por alumno, validado contra la cuenta) ──

#[derive(Serialize, sqlx::FromRow)]
pub struct GradeRow { pub id: i64, pub subject: String, pub evaluation: String, pub grade_text: String, pub grade_num: Option<f64>, pub observations: Option<String>, pub teacher: Option<String>, pub recorded_at: Option<i64>, pub first_seen_at: i64 }
#[derive(Serialize, sqlx::FromRow)]
pub struct AttendanceRow { pub id: i64, pub day: String, pub slot: Option<String>, pub subject: Option<String>, pub kind: String, pub justified: i64, pub justification: Option<String> }
#[derive(Serialize, sqlx::FromRow)]
pub struct MessageRow { pub id: i64, pub kind: String, pub subject: String, pub sender: Option<String>, pub body: Option<String>, pub sent_at: Option<i64>, pub read_in_portal: i64, pub read_local_at: Option<i64>, pub first_seen_at: i64 }

#[tauri::command]
pub async fn familias_grades(st: State<'_, AppState>, student_id: i64) -> AppResult<Vec<GradeRow>> {
    accounts::check_student(&st.pool, account_id(&st.pool).await?, student_id).await?;
    Ok(sqlx::query_as("SELECT id,subject,evaluation,grade_text,grade_num,observations,teacher,recorded_at,first_seen_at FROM grade_record WHERE student_id=? AND removed_at IS NULL ORDER BY evaluation, subject").bind(student_id).fetch_all(&st.pool).await?)
}

#[tauri::command]
pub async fn familias_attendance(st: State<'_, AppState>, student_id: i64) -> AppResult<Vec<AttendanceRow>> {
    accounts::check_student(&st.pool, account_id(&st.pool).await?, student_id).await?;
    Ok(sqlx::query_as("SELECT id,day,slot,subject,kind,justified,justification FROM attendance_record WHERE student_id=? AND removed_at IS NULL ORDER BY day DESC, slot").bind(student_id).fetch_all(&st.pool).await?)
}

#[tauri::command]
pub async fn familias_messages(st: State<'_, AppState>, student_id: Option<i64>) -> AppResult<Vec<MessageRow>> {
    let acc = account_id(&st.pool).await?;
    if let Some(s) = student_id { accounts::check_student(&st.pool, acc, s).await?; }
    // Los avisos del centro (sin alumno) se muestran junto a los del alumno elegido; nunca los de otro alumno.
    Ok(sqlx::query_as("SELECT id,kind,subject,sender,body,sent_at,read_in_portal,read_local_at,first_seen_at FROM school_message WHERE account_id=? AND removed_at IS NULL AND (student_id IS NULL OR student_id=?) ORDER BY coalesce(sent_at, first_seen_at) DESC LIMIT 500")
        .bind(acc).bind(student_id).fetch_all(&st.pool).await?)
}

/// Marca un mensaje como leído SOLO en AulaDesk (no se escribe nada en el portal).
#[tauri::command]
pub async fn familias_message_read(st: State<'_, AppState>, id: i64) -> AppResult<()> {
    let acc = account_id(&st.pool).await?;
    sqlx::query("UPDATE school_message SET read_local_at=strftime('%s','now') WHERE id=? AND account_id=? AND read_local_at IS NULL").bind(id).bind(acc).execute(&st.pool).await?;
    Ok(())
}

#[derive(Serialize)]
pub struct Overview { pub grades: i64, pub subjects: i64, pub average: Option<f64>, pub absences: i64, pub delays: i64, pub unjustified: i64, pub unread: i64, pub recent_grades: Vec<GradeRow>, pub next_events: Vec<(String, i64)> }

#[tauri::command]
pub async fn familias_overview(st: State<'_, AppState>, student_id: i64) -> AppResult<Overview> {
    let p = &st.pool;
    accounts::check_student(p, account_id(p).await?, student_id).await?;
    let one = |sql: &'static str| { let p = p.clone(); async move { sqlx::query_scalar::<_, i64>(sql).bind(student_id).fetch_one(&p).await } };
    let average: Option<f64> = sqlx::query_scalar("SELECT avg(grade_num) FROM grade_record WHERE student_id=? AND removed_at IS NULL AND grade_num IS NOT NULL").bind(student_id).fetch_one(p).await?;
    Ok(Overview {
        grades: one("SELECT count(*) FROM grade_record WHERE student_id=? AND removed_at IS NULL").await?,
        subjects: one("SELECT count(DISTINCT subject) FROM grade_record WHERE student_id=? AND removed_at IS NULL").await?,
        average,
        absences: one("SELECT count(*) FROM attendance_record WHERE student_id=? AND removed_at IS NULL AND kind='falta'").await?,
        delays: one("SELECT count(*) FROM attendance_record WHERE student_id=? AND removed_at IS NULL AND kind='retraso'").await?,
        unjustified: one("SELECT count(*) FROM attendance_record WHERE student_id=? AND removed_at IS NULL AND kind='falta' AND justified=0").await?,
        unread: one("SELECT count(*) FROM school_message WHERE student_id=? AND removed_at IS NULL AND read_in_portal=0 AND read_local_at IS NULL").await?,
        recent_grades: sqlx::query_as("SELECT id,subject,evaluation,grade_text,grade_num,observations,teacher,recorded_at,first_seen_at FROM grade_record WHERE student_id=? AND removed_at IS NULL ORDER BY first_seen_at DESC, id DESC LIMIT 5").bind(student_id).fetch_all(p).await?,
        next_events: sqlx::query_as("SELECT title, starts_at FROM calendar_event WHERE source='familias' AND student_id=? AND cancelled_at IS NULL AND starts_at>strftime('%s','now') ORDER BY starts_at LIMIT 5").bind(student_id).fetch_all(p).await?,
    })
}

// ── Acciones de escritura (con confirmación explícita; ver actions.rs) ──

#[tauri::command]
pub async fn familias_action_prepare(st: State<'_, AppState>, student_id: i64, kind: String, params: super::actions::Params) -> AppResult<i64> {
    super::actions::prepare(&st.pool, account_id(&st.pool).await?, student_id, &kind, &params).await
}

#[tauri::command]
pub async fn familias_action_run(st: State<'_, AppState>, id: i64, confirmed: bool) -> AppResult<super::actions::ActionRow> {
    let active = *st.familias_session.lock().unwrap() == SessionState::Active;
    super::actions::run(&st.pool, account_id(&st.pool).await?, id, confirmed, active).await
}

#[tauri::command]
pub async fn familias_action_cancel(st: State<'_, AppState>, id: i64) -> AppResult<()> { super::actions::cancel(&st.pool, account_id(&st.pool).await?, id).await }

#[tauri::command]
pub async fn familias_actions(st: State<'_, AppState>, student_id: i64) -> AppResult<Vec<super::actions::ActionRow>> { super::actions::list(&st.pool, account_id(&st.pool).await?, student_id).await }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::tests::setup;
    use serde_json::json;

    #[test]
    fn sync_rate_limits() {
        assert!(sync_allowed(1000, None, false).is_ok());
        assert!(sync_allowed(1000 + 3599, Some(1000), false).is_err(), "auto: 60 min");
        assert!(sync_allowed(1000 + 3600, Some(1000), false).is_ok());
        assert!(sync_allowed(1000 + 299, Some(1000), true).is_err(), "manual: 5 min");
        assert!(sync_allowed(1000 + 300, Some(1000), true).is_ok());
        assert!(sync_allowed(1000 + 100, Some(1000), true).unwrap_err().contains("min"));
    }

    #[tokio::test]
    async fn invalid_extraction_is_rejected_without_touching_the_db() {
        let (_d, p, acc, _, _) = setup().await;
        let r = apply(&p, acc, "grades", json!({"selector_version":"f1","container_found":false,"student_ref":"ref-a","items":[]})).await;
        assert!(r.is_err());
        let r = apply(&p, acc, "grades", json!({"selector_version":"f1","container_found":true,"student_ref":"ref-a","items":[{"id":"1","subject":"S","evaluation":"E","grade_text":"5"}]})).await.unwrap();
        assert_eq!(r.changed, 1);
        // una extracción rota a continuación no borra lo ya guardado
        assert!(apply(&p, acc, "grades", json!({"selector_version":"f1","container_found":false,"student_ref":"ref-a","items":[]})).await.is_err());
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM grade_record WHERE removed_at IS NULL").fetch_one(&p).await.unwrap();
        assert_eq!(n, 1);
        assert!(apply(&p, acc, "otro", json!({})).await.is_err());
    }

    #[test]
    fn local_recipes_are_validated_and_bundled_ones_load() {
        let d = tempfile::tempdir().unwrap();
        assert!(!load_recipes(d.path()).unwrap().is_empty(), "sin fichero local: las del paquete");
        std::fs::write(d.path().join("familias_recipes.json"), r#"{"version":"f1","recipes":[{"name":"r","steps":[{"goto":"https://evil/"},{"extract":{"extractor":"grades"}}]}]}"#).unwrap();
        assert!(load_recipes(d.path()).is_err(), "una receta local peligrosa se rechaza");
    }
}
