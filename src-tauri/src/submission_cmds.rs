//! Comandos de entregas. Dos vías, ambas con confirmación explícita previa del usuario: abrir la página de Moodle para entregar a mano (`submission_start`)
//! o entregar desde la app (`submission_submit`, que se detiene ante cualquier duda y verifica por SHA-256).
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
    let w = window::get(&app).ok_or("La ventana de Moodle no está abierta.")?;
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
    if matches!(d, submissions::Decision::Confirmed(_)) {
        // Copia opcional en Drive de lo que Moodle acaba de confirmar (segundo plano; los errores solo se registran).
        let (a, fid) = (app.clone(), v.file_id);
        tauri::async_runtime::spawn(async move { crate::drive::backup_confirmed_submission(&a, fid).await; });
    }
    crate::sync::log_history(&st.pool, "info", &format!("comprobación de entrega v{} → {}", v.version, if matches!(d, submissions::Decision::Confirmed(_)) { "confirmada" } else { "pendiente" }), Some("v1"), None).await;
    submissions::get(&st.pool, id).await.map_err(|e| e.to_string())
}

// ───────────── Entrega desde la app ─────────────
const SUBMIT_MAX: usize = 50 * 1024 * 1024;
const CHUNK: usize = 192 * 1024;

fn progress(app: &AppHandle, stage: &str, text: &str) { let _ = tauri::Emitter::emit(app, "submit-progress", serde_json::json!({ "stage": stage, "text": text })); }

/// Entrega el archivo de la versión preparada: lo sube al formulario oficial de Moodle y lo guarda/envía.
/// `confirmed` debe venir de la confirmación explícita del usuario (diálogo con tarea, archivo y hash). Nunca en lote ni automático.
/// Cada etapa se detiene ante cualquier duda (p. ej. la entrega ya tiene archivos) y deja Moodle abierto para terminar a mano.
#[tauri::command]
pub async fn submission_submit(app: AppHandle, id: i64, confirmed: bool, replace: Option<bool>) -> Result<VersionRow, String> {
    if !confirmed { return Err("Falta la confirmación explícita del usuario.".into()); }
    submit_confirmed(&app, id, replace.unwrap_or(false)).await
}

/// Camino común de la entrega (la llama el comando tras la confirmación del usuario y el programador tras la confirmación guardada al programar).
pub async fn submit_confirmed(app: &AppHandle, id: i64, replace: bool) -> Result<VersionRow, String> {
    let st = app.state::<AppState>();
    if *st.session.lock().unwrap() != SessionState::Active { return Err("No hay sesión de Moodle activa.".into()); }
    if st.sync_running.swap(true, Ordering::SeqCst) { return Err("Hay otra operación con Moodle en curso (sincronización o comprobación). Espera a que termine.".into()); }
    let r = submit_inner(app, &st, id, replace).await;
    st.sync_running.store(false, Ordering::SeqCst);
    if let Err(e) = &r {
        tracing::warn!("entrega desde la app detenida: {e}");
        window::set_visible(app, true);   // se deja Moodle a la vista para terminar a mano
        progress(app, "error", e);
    }
    r
}

async fn wait_load(w: &tauri::Webview, loads: &mut tokio::sync::watch::Receiver<u64>, rx: &mut tokio::sync::oneshot::Receiver<crate::state::IngestOutcome>, what: &str) -> Result<bool, String> {
    // true = la página navegó; false = la etapa respondió «ok» sin navegar. Un fallo de la etapa llega como Err.
    let _ = w;
    tokio::select! {
        r = rx => match r { Ok(Ok(_)) => Ok(false), Ok(Err(e)) => Err(e), Err(_) => Err("etapa cancelada".into()) },
        c = loads.changed() => { c.map_err(|e| e.to_string())?; tokio::time::sleep(std::time::Duration::from_millis(700)).await; Ok(true) }
        _ = tokio::time::sleep(std::time::Duration::from_secs(45)) => Err(format!("Moodle no respondió ({what}).")),
    }
}

async fn submit_inner(app: &AppHandle, st: &AppState, id: i64, replace: bool) -> Result<VersionRow, String> {
    use base64::Engine;
    let v = submissions::get(&st.pool, id).await.map_err(|e| e.to_string())?;
    if !matches!(v.state.as_str(), "prepared" | "in_progress" | "pending_confirmation") { return Err(format!("Esta versión está en estado «{}» y no se puede entregar.", v.state)); }
    // 1) El archivo que se va a subir es EXACTAMENTE el preparado.
    progress(app, "check", "Comprobando el archivo preparado…");
    let files_root = st.data_dir.join("files");
    let path = crate::paths::safe_join(&files_root, std::path::Path::new(&v.rel_path)).map_err(|e| e.to_string())?;
    let bytes = tokio::fs::read(&path).await.map_err(|_| "El archivo preparado ya no está en disco.".to_string())?;
    if bytes.is_empty() || bytes.len() > SUBMIT_MAX { return Err(format!("El archivo debe pesar entre 1 B y {} MB para entregarlo desde la app.", SUBMIT_MAX / 1024 / 1024)); }
    if crate::files::sha256_bytes(&bytes) != v.sha256 { return Err("El archivo preparado ha cambiado en disco desde que lo preparaste. Prepáralo de nuevo.".into()); }
    let (cmid, can): (i64, Option<i64>) = sqlx::query_as("SELECT a.moodle_cmid, x.can_submit FROM activity a LEFT JOIN assignment x ON x.activity_id=a.id WHERE a.id=?")
        .bind(v.activity_id).fetch_one(&st.pool).await.map_err(|e| e.to_string())?;
    if can == Some(0) { return Err("Moodle no ofrece entregar esta tarea ahora (plazo cerrado o sin permiso). Sincroniza para actualizar su estado.".into()); }
    submissions::set_in_progress(&st.pool, id).await.map_err(|e| e.to_string())?;

    // 2) Formulario oficial de entrega
    progress(app, "open", "Abriendo la entrega en Moodle…");
    window::open(app, &st.data_dir, false).await.map_err(|e| e.to_string())?;
    let w = window::get(&app).ok_or("La ventana de Moodle no está abierta.")?;
    let url = window::base_url().join(&format!("mod/assign/view.php?id={cmid}&action=editsubmission")).map_err(|e| e.to_string())?;
    let mut loads = st.page_loads.subscribe();
    loads.borrow_and_update();
    w.navigate(url.clone()).map_err(|e| e.to_string())?;
    crate::library::wait_page(&w, &mut loads, url.path(), false).await?;

    // 3) Subida: el archivo viaja de Rust a la página en trozos (la página nunca puede pedir archivos del equipo)
    progress(app, "upload", "Subiendo el archivo a Moodle…");
    let name = serde_json::to_string(&v.file_name).map_err(|e| e.to_string())?;
    w.eval(format!("window.__mdSubmit && window.__mdSubmit.begin({name}, {replace})")).map_err(|e| e.to_string())?;
    for c in bytes.chunks(CHUNK) {
        w.eval(format!("window.__mdSubmit.chunk('{}')", base64::engine::general_purpose::STANDARD.encode(c))).map_err(|e| e.to_string())?;
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    *st.pending.lock().unwrap() = Some(tx);
    w.eval("window.__mdRun && window.__mdRun('submit_stage')").map_err(|e| e.to_string())?;
    match tokio::time::timeout(std::time::Duration::from_secs(120), rx).await {
        Ok(Ok(Ok(_))) => {}
        Ok(Ok(Err(e))) => return Err(e),
        _ => { st.pending.lock().unwrap().take(); return Err("Moodle no confirmó la subida del archivo a tiempo.".into()); }
    }

    // 4) Guardar → (si hace falta) Enviar tarea → Continuar
    for (stage, text, what) in [("submit_save", "Guardando la entrega…", "guardar"), ("submit_final", "Enviando la tarea para calificar…", "enviar"), ("submit_confirm", "Confirmando el envío…", "confirmar")] {
        progress(app, stage, text);
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        *st.pending.lock().unwrap() = Some(tx);
        loads.borrow_and_update();
        w.eval(format!("window.__mdRun && window.__mdRun('{stage}')")).map_err(|e| e.to_string())?;
        let navigated = wait_load(&w, &mut loads, &mut rx, what).await;
        st.pending.lock().unwrap().take();
        match navigated {
            Ok(true) => {}
            Ok(false) => { if stage == "submit_final" { break; } }   // sin paso de envío: la tarea ya quedó entregada al guardar
            Err(e) => return Err(e),
        }
    }

    // 5) La única fuente de verdad: lo que Moodle muestra, comparado por SHA-256
    progress(app, "verify", "Verificando con Moodle (SHA-256)…");
    let r = verify_inner(app, st, id).await?;
    progress(app, "done", if r.state == "confirmed" { "Entrega confirmada por Moodle." } else { "Entrega enviada; Moodle aún no muestra el archivo idéntico." });
    Ok(r)
}
