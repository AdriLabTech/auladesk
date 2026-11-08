//! Biblioteca: descargas orquestadas con la sesión del WebView y gestión de archivos locales.
use crate::downloads::{self, Stored};
use crate::error::{AppError, AppResult};
use crate::moodle::{session::SessionState, window};
use crate::paths::{safe_join, sanitize_component};
use crate::state::{AppState, DlCtx};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

fn files_root(st: &AppState) -> PathBuf { st.data_dir.join("files") }
fn other(e: impl ToString) -> AppError { AppError::Other(e.to_string()) }

// ---------------- descargas ----------------

/// Llamado por el manejador de descargas del WebView cuando termina una descarga.
pub async fn on_download_finished(app: AppHandle, url: String, path: Option<PathBuf>, success: bool) {
    let st = app.state::<AppState>();
    let ctx = st.dl.lock().unwrap().as_ref().map(|c| (c.activity_id, c.tx.clone(), c.hash_only));
    let name = st.dl_name.lock().unwrap().take().unwrap_or_default();
    // Modo «solo hash»: se descarga para comparar el contenido (verificar una entrega) y no se guarda nada en la biblioteca.
    if let (Some((_, tx, true)), true, Some(p)) = (&ctx, success, &path) {
        let r = downloads::hash_and_head(p).await.map(|(sha, size, _)| Stored { sha256: sha, file_id: 0, rel_path: String::new(), original_name: downloads::name_from_url(&url), duplicate: false, new_version: false, size }).map_err(|e| e.to_string());
        let _ = tokio::fs::remove_file(p).await;
        let _ = tx.send(r);
        return;
    }
    let res: Result<Stored, String> = match (success, path) {
        (true, Some(p)) => downloads::ingest(&st.pool, &files_root(&st), &p, &url, &name, ctx.as_ref().and_then(|c| c.0)).await.map_err(|e| e.to_string()),
        (_, p) => { if let Some(p) = p { let _ = tokio::fs::remove_file(p).await; } Err("la descarga se interrumpió".into()) }
    };
    match &res {
        Ok(s) => { if !s.duplicate { crate::reminders::emit(&st.pool, "download", 0, "Documento descargado", Some(&s.original_name), None, None).await; } tracing::info!("descarga guardada: {} (duplicado={}, versión nueva={})", s.rel_path, s.duplicate, s.new_version); let _ = app.emit("download-done", s); }
        Err(e) => { tracing::warn!("descarga rechazada: {e}"); crate::sync::log_history(&st.pool, "error", &format!("descarga: {e}"), None, None).await; }
    }
    if let Some((_, tx, _)) = ctx { let _ = tx.send(res); }
}

fn with_forcedownload(u: &str) -> String { if u.contains('?') { format!("{u}&forcedownload=1") } else { format!("{u}?forcedownload=1") } }

#[derive(Serialize, Default)]
pub struct DownloadReport { pub stored: Vec<Stored>, pub errors: Vec<String> }

/// Espera el resultado de una descarga. WebKit emite «página cargada» antes de que empiece la descarga,
/// así que no se usa como señal de fallo: se da un período de gracia y, si la descarga empezó, se espera a que acabe.
pub(crate) async fn await_download(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Result<Stored, String>>, st: &AppState) -> Result<Result<Stored, String>, String> {
    use std::sync::atomic::Ordering;
    match tokio::time::timeout(Duration::from_secs(15), rx.recv()).await {
        Ok(Some(r)) => return Ok(r),
        Ok(None) => return Err("canal de descarga cerrado".into()),
        Err(_) => {}
    }
    if !st.dl_started.load(Ordering::SeqCst) { return Err("el servidor no inició la descarga (¿sesión caducada o archivo mostrado en vez de descargado?)".into()); }
    match tokio::time::timeout(Duration::from_secs(600), rx.recv()).await {
        Ok(Some(r)) => Ok(r),
        Ok(None) => Err("canal de descarga cerrado".into()),
        Err(_) => Err("tiempo agotado esperando el fin de la descarga".into()),
    }
}

/// Espera a que el WebView esté realmente en la página pedida. Los eventos «página cargada» pueden llegar rezagados
/// (p. ej. de la descarga anterior), así que tras cada uno se comprueba la URL actual.
pub(crate) async fn wait_page(w: &tauri::WebviewWindow, loads: &mut tokio::sync::watch::Receiver<u64>, expect_path: &str, accept_file: bool) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(45);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        tokio::time::timeout(left, loads.changed()).await.map_err(|_| "La página de la actividad no cargó".to_string())?.map_err(|e| e.to_string())?;
        if let Ok(u) = w.url() {
            let ok = u.path() == expect_path || (accept_file && u.path().contains("/pluginfile.php/"));
            if ok && window::is_allowed(&u) { return Ok(()); }
            tracing::info!("evento de carga ignorado (aún no es la página pedida): {}", u.path());
        }
    }
}

async fn do_download_activity(app: &AppHandle, st: &AppState, activity_id: i64) -> Result<DownloadReport, String> {
    if *st.session.lock().unwrap() != SessionState::Active { return Err("No hay sesión de Moodle activa. Inicia sesión primero.".into()); }
    let w = app.get_webview_window(window::LABEL).ok_or("La ventana de Moodle no está abierta: ábrela en «Moodle / Conexión» e inicia sesión.")?;
    let (url, modname): (Option<String>, String) = sqlx::query_as("SELECT url, modname FROM activity WHERE id=?").bind(activity_id).fetch_optional(&st.pool).await
        .map_err(|e| e.to_string())?.ok_or("Actividad desconocida")?;
    let url = url.ok_or("Esta actividad no tiene enlace")?;
    if !matches!(modname.as_str(), "resource" | "folder" | "assign") { return Err(format!("Las actividades de tipo «{modname}» no tienen archivos descargables.")); }

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    *st.dl.lock().unwrap() = Some(DlCtx { hash_only: false, activity_id: Some(activity_id), tx });
    let result = async {
        let mut report = DownloadReport::default();
        let (ftx, frx) = tokio::sync::oneshot::channel();
        *st.found_files.lock().unwrap() = Some(ftx);
        tokio::time::sleep(Duration::from_millis(1500)).await; // deja pasar eventos rezagados de la operación anterior
        let target = tauri::Url::parse(&url).map_err(|e| e.to_string())?;
        let mut loads = st.page_loads.subscribe();
        loads.borrow_and_update();
        w.navigate(target.clone()).map_err(|e| e.to_string())?;
        tokio::select! {
            r = rx.recv() => { // la propia página de recurso redirigió a una descarga directa
                match r { Some(Ok(s)) => report.stored.push(s), Some(Err(e)) => report.errors.push(e), None => {} }
                return Ok(report);
            }
            r = wait_page(&w, &mut loads, target.path(), modname == "resource") => { r?; }
        }
        w.eval("window.__mdRun && window.__mdRun('files')").map_err(|e| e.to_string())?;
        let files = tokio::time::timeout(Duration::from_secs(30), frx).await.map_err(|_| "El extractor de archivos no respondió".to_string())?
            .map_err(|_| "cancelado".to_string())??;
        if files.is_empty() { return Err("Esta actividad no tiene archivos adjuntos del profesorado (tus propias entregas no se descargan aquí).".into()); }
        for (name, furl) in files {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            st.dl_started.store(false, std::sync::atomic::Ordering::SeqCst);
            w.navigate(tauri::Url::parse(&with_forcedownload(&furl)).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            match await_download(&mut rx, st).await {
                Ok(Ok(s)) => report.stored.push(s),
                Ok(Err(e)) => report.errors.push(format!("{name}: {e}")),
                Err(e) => report.errors.push(format!("{name}: {e}")),
            }
        }
        Ok(report)
    }.await;
    *st.dl.lock().unwrap() = None;
    *st.found_files.lock().unwrap() = None;
    result
}

#[tauri::command]
pub async fn download_activity(app: AppHandle, activity_id: i64) -> Result<DownloadReport, String> {
    let st = app.state::<AppState>();
    if st.sync_running.swap(true, std::sync::atomic::Ordering::SeqCst) { return Err("Hay otra operación con Moodle en curso.".into()); }
    let r = do_download_activity(&app, &st, activity_id).await;
    st.sync_running.store(false, std::sync::atomic::Ordering::SeqCst);
    r
}

#[derive(Serialize)]
pub struct CourseDownloadReport { pub activities: i64, pub stored: i64, pub duplicates: i64, pub errors: Vec<String> }

#[tauri::command]
pub async fn download_course(app: AppHandle, course_id: i64, only_missing: bool) -> Result<CourseDownloadReport, String> {
    let st = app.state::<AppState>();
    if st.sync_running.swap(true, std::sync::atomic::Ordering::SeqCst) { return Err("Hay otra operación con Moodle en curso.".into()); }
    let acts: Result<Vec<(i64, String)>, _> = sqlx::query_as(
        "SELECT a.id, a.name FROM activity a WHERE a.course_id=?1 AND a.removed_at IS NULL AND a.modname IN ('resource','folder','assign') AND a.url IS NOT NULL
         AND (?2 = 0 OR NOT EXISTS (SELECT 1 FROM file_association f WHERE f.activity_id=a.id)) ORDER BY a.id").bind(course_id).bind(only_missing as i64).fetch_all(&st.pool).await;
    let acts = match acts { Ok(a) => a, Err(e) => { st.sync_running.store(false, std::sync::atomic::Ordering::SeqCst); return Err(e.to_string()); } };
    let mut rep = CourseDownloadReport { activities: acts.len() as i64, stored: 0, duplicates: 0, errors: vec![] };
    for (i, (id, name)) in acts.iter().enumerate() {
        let _ = app.emit("download-progress", serde_json::json!({ "done": i, "total": acts.len(), "current": name }));
        match do_download_activity(&app, &st, *id).await {
            Ok(r) => { for s in r.stored { if s.duplicate { rep.duplicates += 1 } else { rep.stored += 1 } } rep.errors.extend(r.errors.into_iter().map(|e| format!("{name}: {e}"))); }
            Err(e) => rep.errors.push(format!("{name}: {e}")),
        }
    }
    let _ = app.emit("download-progress", serde_json::json!({ "done": acts.len(), "total": acts.len(), "current": null }));
    st.sync_running.store(false, std::sync::atomic::Ordering::SeqCst);
    Ok(rep)
}

// ---------------- portadas ----------------

pub fn cover_ext(mime: &str) -> Option<&'static str> {
    match mime { "image/svg+xml" => Some("svg"), "image/png" => Some("png"), "image/jpeg" => Some("jpg"), "image/webp" => Some("webp"), "image/gif" => Some("gif"), _ => None }
}

/// Guarda la portada (data URI) de un curso. Devuelve la ruta relativa a cache/covers.
pub async fn save_cover(data_dir: &Path, moodle_id: i64, data_uri: &str) -> Option<String> {
    use base64::Engine;
    let rest = data_uri.strip_prefix("data:")?;
    let (meta, b64) = rest.split_once(',')?;
    let mime = meta.strip_suffix(";base64")?;
    let ext = cover_ext(mime)?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    if bytes.is_empty() || bytes.len() > 400 * 1024 { return None; }
    let dir = data_dir.join("cache/covers");
    tokio::fs::create_dir_all(&dir).await.ok()?;
    let name = format!("{moodle_id}.{ext}");
    tokio::fs::write(dir.join(&name), bytes).await.ok()?;
    Some(name)
}

#[tauri::command]
pub async fn course_cover(st: State<'_, AppState>, course_id: i64) -> AppResult<Option<String>> {
    use base64::Engine;
    let rel: Option<String> = sqlx::query_scalar("SELECT cover_path FROM course WHERE id=?").bind(course_id).fetch_optional(&st.pool).await?.flatten();
    let Some(rel) = rel else { return Ok(None) };
    let path = safe_join(&st.data_dir.join("cache/covers"), Path::new(&rel))?;
    let Ok(bytes) = tokio::fs::read(&path).await else { return Ok(None) };
    let mime = match path.extension().and_then(|e| e.to_str()) { Some("svg") => "image/svg+xml", Some("png") => "image/png", Some("jpg") => "image/jpeg", Some("webp") => "image/webp", Some("gif") => "image/gif", _ => return Ok(None) };
    Ok(Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))))
}

// ---------------- gestión de archivos locales ----------------

#[derive(Serialize, sqlx::FromRow)]
struct FileBase { id: i64, original_name: String, local_name: String, rel_path: String, extension: Option<String>, size_bytes: i64, sha256: String, source_url: Option<String>,
    downloaded_at: i64, sync_state: String, course_id: Option<i64>, course_name: Option<String>, section_position: Option<i64>, activity_id: Option<i64>, activity_name: Option<String> }

#[derive(Serialize)]
pub struct FileRow { #[serde(flatten)] base: FileBase, kind: &'static str, exists: bool }

pub fn kind_of(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "pdf" => "pdf", "doc" | "docx" | "odt" | "rtf" | "txt" | "md" => "documento", "xls" | "xlsx" | "ods" | "csv" => "hoja",
        "ppt" | "pptx" | "odp" => "presentación", "zip" | "rar" | "7z" | "tar" | "gz" => "comprimido",
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => "imagen", "java" | "py" | "rs" | "cs" | "js" | "ts" | "sql" | "xml" | "json" | "html" | "css" => "código",
        _ => "otro",
    }
}

#[derive(Deserialize, Default)]
pub struct FileFilter { pub activity_id: Option<i64>, pub course_id: Option<i64>, pub kind: Option<String>, pub q: Option<String>, pub sort: Option<String>, pub state: Option<String> }

const FILE_SELECT: &str = "SELECT l.id, l.original_name, l.local_name, l.rel_path, l.extension, l.size_bytes, l.sha256, l.source_url, l.downloaded_at, l.sync_state,
   (SELECT a.course_id FROM file_association a WHERE a.file_id=l.id ORDER BY a.id LIMIT 1) AS course_id,
   (SELECT c.fullname FROM file_association a JOIN course c ON c.id=a.course_id WHERE a.file_id=l.id ORDER BY a.id LIMIT 1) AS course_name,
   (SELECT s.position FROM file_association a JOIN course_section s ON s.id=a.section_id WHERE a.file_id=l.id ORDER BY a.id LIMIT 1) AS section_position,
   (SELECT a.activity_id FROM file_association a WHERE a.file_id=l.id ORDER BY a.id LIMIT 1) AS activity_id,
   (SELECT ac.name FROM file_association a JOIN activity ac ON ac.id=a.activity_id WHERE a.file_id=l.id ORDER BY a.id LIMIT 1) AS activity_name
   FROM local_file l";

fn to_rows(st: &AppState, base: Vec<FileBase>) -> Vec<FileRow> {
    let root = files_root(st);
    base.into_iter().map(|b| { let kind = kind_of(b.extension.as_deref().unwrap_or("")); let exists = root.join(&b.rel_path).exists(); FileRow { base: b, kind, exists } }).collect()
}

#[tauri::command]
pub async fn list_files(st: State<'_, AppState>, filter: FileFilter) -> AppResult<Vec<FileRow>> {
    let order = match filter.sort.as_deref() { Some("name") => "l.original_name COLLATE NOCASE", Some("size") => "l.size_bytes DESC", Some("type") => "l.extension, l.original_name", _ => "l.downloaded_at DESC, l.id DESC" };
    let base: Vec<FileBase> = sqlx::query_as(&format!("{FILE_SELECT} ORDER BY {order}")).fetch_all(&st.pool).await?;
    let q = filter.q.as_deref().map(|s| s.to_lowercase()).filter(|s| !s.is_empty());
    let rows = to_rows(&st, base).into_iter().filter(|r| {
        filter.activity_id.map_or(true, |a| r.base.activity_id == Some(a))
            && filter.course_id.map_or(true, |c| r.base.course_id == Some(c))
            && filter.kind.as_deref().map_or(true, |k| k.is_empty() || r.kind == k)
            && filter.state.as_deref().map_or(true, |s| s.is_empty() || r.base.sync_state == s)
            && q.as_ref().map_or(true, |q| r.base.original_name.to_lowercase().contains(q) || r.base.rel_path.to_lowercase().contains(q) || r.base.activity_name.as_deref().unwrap_or("").to_lowercase().contains(q))
    }).collect();
    Ok(rows)
}

#[tauri::command]
pub async fn list_duplicates(st: State<'_, AppState>) -> AppResult<Vec<Vec<FileRow>>> {
    let base: Vec<FileBase> = sqlx::query_as(&format!("{FILE_SELECT} WHERE l.sha256 IN (SELECT sha256 FROM local_file GROUP BY sha256 HAVING count(*)>1) ORDER BY l.sha256, l.id")).fetch_all(&st.pool).await?;
    let mut groups: Vec<Vec<FileRow>> = vec![];
    let mut last = String::new();
    for r in to_rows(&st, base) { if r.base.sha256 != last { groups.push(vec![]); last = r.base.sha256.clone(); } groups.last_mut().unwrap().push(r); }
    Ok(groups)
}

async fn path_of(st: &AppState, id: i64) -> AppResult<(PathBuf, String)> {
    let rel: String = sqlx::query_scalar("SELECT rel_path FROM local_file WHERE id=?").bind(id).fetch_optional(&st.pool).await?.ok_or_else(|| other("archivo desconocido"))?;
    Ok((safe_join(&files_root(st), Path::new(&rel))?, rel))
}

#[tauri::command]
pub async fn file_open(app: AppHandle, st: State<'_, AppState>, id: i64) -> AppResult<()> {
    let (p, _) = path_of(&st, id).await?;
    if !p.exists() { return Err(other("El archivo ya no está en disco.")); }
    app.opener().open_path(p.to_string_lossy(), None::<&str>).map_err(other)
}

/// Tamaño máximo que se envía al visor integrado; por encima se ofrece el visor del sistema.
const VIEW_MAX: u64 = 80 * 1024 * 1024;

/// Bytes de un archivo de la biblioteca para el visor. La ruta sale de la BD y pasa por `safe_join` (nunca una ruta que dé la interfaz).
#[tauri::command]
pub async fn file_read(st: State<'_, AppState>, id: i64) -> AppResult<tauri::ipc::Response> {
    let (p, _) = path_of(&st, id).await?;
    let len = tokio::fs::metadata(&p).await.map_err(|_| other("El archivo ya no está en disco."))?.len();
    if len > VIEW_MAX { return Err(other("Archivo demasiado grande para el visor (máx. 80 MB). Ábrelo con el visor del sistema.")); }
    Ok(tauri::ipc::Response::new(tokio::fs::read(&p).await.map_err(other)?))
}

#[tauri::command]
pub async fn file_reveal(app: AppHandle, st: State<'_, AppState>, id: i64) -> AppResult<()> {
    let (p, _) = path_of(&st, id).await?;
    if !p.exists() { return Err(other("El archivo ya no está en disco.")); }
    app.opener().reveal_item_in_dir(p).map_err(other)
}

async fn relocate(st: &AppState, id: i64, new_rel: PathBuf) -> AppResult<String> {
    let (old, _) = path_of(st, id).await?;
    let new_full = safe_join(&files_root(st), &new_rel)?;
    if new_full.exists() { return Err(other("Ya existe un archivo con ese nombre en el destino.")); }
    if let Some(d) = new_full.parent() { tokio::fs::create_dir_all(d).await?; }
    tokio::fs::rename(&old, &new_full).await?;
    let rel = new_rel.to_string_lossy().to_string();
    let name = new_rel.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
    if let Err(e) = sqlx::query("UPDATE local_file SET rel_path=?, local_name=? WHERE id=?").bind(&rel).bind(&name).bind(id).execute(&st.pool).await {
        let _ = tokio::fs::rename(&new_full, &old).await; // coherencia disco/BD
        return Err(e.into());
    }
    Ok(rel)
}

#[tauri::command]
pub async fn file_rename(st: State<'_, AppState>, id: i64, new_name: String) -> AppResult<String> {
    let (_, rel) = path_of(&st, id).await?;
    let clean = downloads::sanitize_filename(new_name.trim());
    if clean.is_empty() { return Err(other("Nombre no válido")); }
    let new_rel = Path::new(&rel).parent().unwrap_or(Path::new("")).join(clean);
    relocate(&st, id, new_rel).await
}

fn clean_dir(dir: &str) -> AppResult<PathBuf> {
    let parts: Vec<String> = dir.split('/').filter(|s| !s.trim().is_empty()).map(sanitize_component).collect();
    if parts.is_empty() { return Err(other("Carpeta no válida")); }
    Ok(parts.iter().collect())
}

#[tauri::command]
pub async fn file_move(st: State<'_, AppState>, id: i64, dest_dir: String) -> AppResult<String> {
    let (_, rel) = path_of(&st, id).await?;
    let name = Path::new(&rel).file_name().map(|n| n.to_owned()).ok_or_else(|| other("ruta inválida"))?;
    relocate(&st, id, clean_dir(&dest_dir)?.join(name)).await
}

#[tauri::command]
pub async fn folder_create(st: State<'_, AppState>, dir: String) -> AppResult<String> {
    let rel = clean_dir(&dir)?;
    tokio::fs::create_dir_all(safe_join(&files_root(&st), &rel)?).await?;
    Ok(rel.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn list_folders(st: State<'_, AppState>) -> AppResult<Vec<String>> {
    let mut out: Vec<String> = sqlx::query_scalar("SELECT DISTINCT rel_path FROM local_file").fetch_all(&st.pool).await?
        .into_iter().filter_map(|r: String| Path::new(&r).parent().map(|p| p.to_string_lossy().to_string())).filter(|p| !p.is_empty()).collect();
    // carpetas vacías creadas por el usuario (dos niveles bastan para el selector)
    let root = files_root(&st);
    if let Ok(mut rd) = tokio::fs::read_dir(&root).await {
        while let Ok(Some(e)) = rd.next_entry().await {
            if e.path().is_dir() { out.push(e.file_name().to_string_lossy().to_string()); }
        }
    }
    out.sort(); out.dedup();
    Ok(out)
}

/// La eliminación es solo local: nunca toca Moodle. Se rechaza si el archivo es parte del historial de entregas.
#[tauri::command]
pub async fn file_delete(st: State<'_, AppState>, id: i64) -> AppResult<()> {
    let (p, _) = path_of(&st, id).await?;
    let used: i64 = sqlx::query_scalar("SELECT count(*) FROM submission_version WHERE file_id=?").bind(id).fetch_one(&st.pool).await?;
    if used > 0 { return Err(other("Este archivo forma parte del historial de entregas y no se puede eliminar.")); }
    // Otros registros con el mismo contenido en la misma ruta no existen (rel_path UNIQUE): se borra archivo y fila.
    if p.exists() { tokio::fs::remove_file(&p).await?; }
    sqlx::query("DELETE FROM local_file WHERE id=?").bind(id).execute(&st.pool).await?;
    Ok(())
}

/// Copia un archivo propio a la biblioteca (opcionalmente asociado a una actividad).
#[tauri::command]
pub async fn import_file(st: State<'_, AppState>, path: String, activity_id: Option<i64>) -> AppResult<Stored> {
    let src = PathBuf::from(&path);
    if !src.is_file() { return Err(other("No es un archivo")); }
    let name = src.file_name().and_then(|n| n.to_str()).unwrap_or("archivo").to_string();
    let tmp = st.data_dir.join("cache/downloads").join(format!("{}.part", uuid::Uuid::new_v4()));
    if let Some(d) = tmp.parent() { tokio::fs::create_dir_all(d).await?; }
    tokio::fs::copy(&src, &tmp).await?;
    downloads::ingest(&st.pool, &files_root(&st), &tmp, &format!("file://local/{name}"), &name, activity_id).await
}

pub async fn _unused(_: &SqlitePool) {}

/// Descarga `url` con la sesión del WebView solo para calcular su SHA-256 (no se guarda en la biblioteca).
pub async fn download_hash(app: &AppHandle, st: &AppState, url: &str) -> Result<(String, i64), String> {
    let w = app.get_webview_window(window::LABEL).ok_or("La ventana de Moodle no está abierta.")?;
    let parsed = tauri::Url::parse(url).map_err(|e| e.to_string())?;
    if !window::is_allowed(&parsed) { return Err("URL fuera de Moodle".into()); }
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    *st.dl.lock().unwrap() = Some(DlCtx { hash_only: true, activity_id: None, tx });
    tokio::time::sleep(Duration::from_millis(1000)).await;
    st.dl_started.store(false, std::sync::atomic::Ordering::SeqCst);
    let r = async {
        w.navigate(tauri::Url::parse(&with_forcedownload(url)).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        match await_download(&mut rx, st).await { Ok(Ok(s)) => Ok((s.sha256, s.size)), Ok(Err(e)) | Err(e) => Err(e) }
    }.await;
    *st.dl.lock().unwrap() = None;
    r
}

/// Solo desarrollo: descarga una URL de Moodle y la guarda en «Sin_clasificar» (sin asociarla a ninguna actividad).
pub async fn fetch_url(app: &AppHandle, st: &AppState, url: &str) -> Result<Stored, String> {
    let w = app.get_webview_window(window::LABEL).ok_or("La ventana de Moodle no está abierta.")?;
    let parsed = tauri::Url::parse(url).map_err(|e| e.to_string())?;
    if !window::is_allowed(&parsed) { return Err("URL fuera de Moodle".into()); }
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    *st.dl.lock().unwrap() = Some(DlCtx { hash_only: false, activity_id: None, tx });
    tokio::time::sleep(Duration::from_millis(1000)).await;
    st.dl_started.store(false, std::sync::atomic::Ordering::SeqCst);
    let r = async {
        w.navigate(tauri::Url::parse(&with_forcedownload(url)).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        match await_download(&mut rx, st).await { Ok(Ok(s)) => Ok(s), Ok(Err(e)) | Err(e) => Err(e) }
    }.await;
    *st.dl.lock().unwrap() = None;
    r
}
