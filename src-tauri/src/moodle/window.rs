//! Ventana de Moodle: WebView con almacén de cookies propio y persistente. Aquí no se leen ni copian cookies.
use crate::error::{AppError, AppResult};
use tauri::webview::{DownloadEvent, PageLoadPayload};
use tauri::{AppHandle, Emitter, Manager, Url, Webview, WebviewUrl};

#[cfg(target_os = "linux")]
pub(crate) mod embedded;

pub const LABEL: &str = "moodle";
const WORKER_PREFIX: &str = "moodle-w";
pub fn worker_label(i: usize) -> String { format!("{WORKER_PREFIX}{i}") }
/// Índice de una ventana de lectura en paralelo, si `label` es una de ellas.
pub fn worker_index(label: &str) -> Option<usize> { label.strip_prefix(WORKER_PREFIX)?.parse().ok().filter(|i| *i < crate::state::WORKERS) }
pub const DEFAULT_BASE: &str = "https://educacionadistancia.juntadeandalucia.es/centros/cordoba/";
const BRIDGE: &str = include_str!("../../selectors/bridge.js");

pub fn base_url() -> Url { Url::parse(DEFAULT_BASE).expect("base válida") }

/// Navegación permitida en la ventana: Moodle y la pasarela de identidad de la Junta (Acceso Único Educación, CAS).
/// El IPC sigue limitado a la ruta de Moodle (capabilities); aquí solo se decide qué páginas se pueden mostrar.
pub fn nav_allowed(u: &Url) -> bool {
    u.as_str() == "about:blank" || is_allowed(u) || (u.scheme() == "https" && u.host_str().is_some_and(|h| h.ends_with(".juntadeandalucia.es")))
}

/// Solo se permite leer/navegar por programa dentro del sitio de Moodle configurado.
pub fn is_allowed(u: &Url) -> bool {
    let b = base_url();
    u.scheme() == "https" && u.host_str() == b.host_str() && u.path().starts_with(b.path())
}

/// Webview de Moodle (en Linux es un webview dentro de la ventana principal; en el resto, una ventana propia).
pub fn get(app: &AppHandle) -> Option<Webview> { app.get_webview(LABEL) }

/// Muestra u oculta Moodle. Oculto sigue cargando y sincronizando; visible es para iniciar sesión o terminar algo a mano.
pub fn set_visible(app: &AppHandle, visible: bool) {
    #[cfg(target_os = "linux")]
    embedded::set_visible(app, LABEL, visible);
    #[cfg(not(target_os = "linux"))]
    if let Some(w) = app.get_webview_window(LABEL) {
        if visible { let _ = w.show(); let _ = w.set_focus(); } else { let _ = w.hide(); }
    }
}

/// Cookies guardadas en el llavero, si el usuario no ha desactivado recordar la sesión.
async fn saved_session(app: &AppHandle) -> Option<Vec<crate::session_store::StoredCookie>> {
    let saved = tauri::async_runtime::spawn_blocking(crate::session_store::load).await.ok().flatten();
    if saved.is_some() && remember_enabled(app).await { saved } else { None }
}

/// Con sesión guardada se carga about:blank, se restauran las cookies y entonces se navega a Moodle.
fn restore_session(win: &Webview, cookies: &[crate::session_store::StoredCookie]) -> AppResult<()> {
    for c in cookies {
        if let Err(e) = win.set_cookie(crate::session_store::to_cookie(c)) { tracing::warn!("no se pudo restaurar una cookie: {e}"); }
    }
    tracing::info!("sesión guardada restaurada ({} cookies); comprobando con Moodle", cookies.len());
    win.navigate(base_url().join("my/").map_err(|e| AppError::Other(e.to_string()))?).map_err(|e| AppError::Other(e.to_string()))
}

fn handle_download(app: &AppHandle, ev: DownloadEvent<'_>) -> bool {
    match ev {
        DownloadEvent::Requested { url, destination } => {
            if !nav_allowed(&url) { tracing::warn!("descarga bloqueada fuera de Moodle: host={:?}", url.host_str()); return false; }
            let st = app.state::<crate::state::AppState>();
            *st.dl_name.lock().unwrap() = destination.file_name().map(|n| n.to_string_lossy().to_string());
            st.dl_started.store(true, std::sync::atomic::Ordering::SeqCst);
            let dir = st.data_dir.join("cache/downloads");
            let _ = std::fs::create_dir_all(&dir);
            *destination = dir.join(format!("{}.part", uuid::Uuid::new_v4()));
            tracing::info!("descarga iniciada: {}{}", url.host_str().unwrap_or(""), url.path());
            true
        }
        DownloadEvent::Finished { url, path, success } => {
            tauri::async_runtime::spawn(crate::library::on_download_finished(app.clone(), url.to_string(), path, success));
            true
        }
        _ => true,
    }
}

fn handle_navigation(u: &Url) -> bool {
    let ok = nav_allowed(u);
    if !ok { tracing::warn!("navegación bloqueada fuera de Moodle: host={:?}", u.host_str()); }
    ok
}

fn handle_title(t: &str) {
    if t.starts_with("[mdbridge error") { tracing::warn!("puente: {}", t.split(']').next().unwrap_or("")); }
}

fn handle_load(app: &AppHandle, w: &Webview, p: &PageLoadPayload<'_>) {
    // Solo host+ruta: nunca query ni fragmento (pueden llevar tokens).
    let u = p.url();
    if let Some(i) = worker_index(w.label()) {
        if matches!(p.event(), tauri::webview::PageLoadEvent::Finished) { app.state::<crate::state::AppState>().worker_loads[i].send_modify(|n| *n += 1); }
        return;
    }
    tracing::info!("moodle página {:?}: {}{}", p.event(), u.host_str().unwrap_or(""), u.path());
    let _ = app.emit("moodle-page", u.path().to_string());
    if matches!(p.event(), tauri::webview::PageLoadEvent::Finished) {
        app.state::<crate::state::AppState>().page_loads.send_modify(|n| *n += 1);
        if is_allowed(u) { let _ = w.eval("window.__mdRun && window.__mdRun('session')"); }
    }
}

/// Linux: Moodle es un webview dentro de la ventana principal (ver `embedded`), sin segunda ventana.
#[cfg(target_os = "linux")]
pub async fn open(app: &AppHandle, data_dir: &std::path::Path, visible: bool) -> AppResult<()> {
    embedded::open(app, data_dir, visible).await
}

/// Ventanas de lectura en paralelo: solo Linux. En el resto se sincroniza con la ventana principal sola.
#[cfg(target_os = "linux")]
pub async fn open_worker(app: &AppHandle, data_dir: &std::path::Path, i: usize) -> AppResult<Webview> { embedded::open_worker(app, data_dir, i).await }
#[cfg(not(target_os = "linux"))]
pub async fn open_worker(_app: &AppHandle, _data_dir: &std::path::Path, _i: usize) -> AppResult<Webview> { Err(AppError::Other("lectura en paralelo no disponible en esta plataforma".into())) }
pub fn close_worker(app: &AppHandle, i: usize) {
    #[cfg(target_os = "linux")]
    embedded::close_worker(app, i);
    #[cfg(not(target_os = "linux"))]
    if let Some(w) = app.get_webview(&worker_label(i)) { let _ = w.close(); }
}

/// Resto de plataformas: ventana propia con almacén de cookies persistente.
#[cfg(not(target_os = "linux"))]
pub async fn open(app: &AppHandle, data_dir: &std::path::Path, visible: bool) -> AppResult<()> {
    use tauri::WebviewWindowBuilder;
    if let Some(w) = app.get_webview_window(LABEL) {
        if !visible { return Ok(()); }
        w.show().map_err(|e| AppError::Other(e.to_string()))?;
        w.set_focus().ok();
        return Ok(());
    }
    let store = data_dir.join("webview");
    std::fs::create_dir_all(&store)?;
    let saved = saved_session(app).await;
    let initial = if saved.is_some() { Url::parse("about:blank").unwrap() } else { base_url() };
    let emitter = app.clone();
    let dl_app = app.clone();
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::External(initial))
        .title("Moodle — AulaDesk")
        .visible(visible)
        .focused(false)
        .inner_size(1100.0, 800.0)
        .data_directory(store)
        .initialization_script(BRIDGE)
        // Sin esto, Tauri captura los archivos soltados sobre la ventana y la página de Moodle (selector de archivos de la entrega) nunca los recibe.
        .disable_drag_drop_handler()
        .on_navigation(handle_navigation)
        .on_download(move |_w, ev| handle_download(&dl_app, ev))
        .on_document_title_changed(|_w, t| handle_title(&t))
        .on_page_load(move |w, p| handle_load(&emitter, w.as_ref(), &p))
        .build()
        .map_err(|e| AppError::Other(format!("no se pudo abrir Moodle: {e}")))?;
    if let Some(cookies) = saved { restore_session(win.as_ref(), &cookies)?; }
    // WebKit no renderiza las páginas dinámicas de Moodle en una ventana que nunca se ha mostrado: se muestra un instante y se oculta.
    if !visible {
        let w3 = win.clone();
        let app2 = app.clone();
        let mut loads = app.state::<crate::state::AppState>().page_loads.subscribe();
        loads.borrow_and_update();
        tauri::async_runtime::spawn(async move {
            let _ = w3.show();
            // Hasta que termine de cargar la página de Moodle y se haya pintado; si no llega, se oculta igualmente.
            let _ = tokio::time::timeout(std::time::Duration::from_secs(20), async {
                while loads.changed().await.is_ok() {
                    if w3.url().map(|u| is_allowed(&u)).unwrap_or(false) { break; }
                }
            }).await;
            tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
            let _ = w3.hide();
            let _ = app2.emit("moodle-window-ready", true);
        });
    }
    // Cerrar la ventana solo la oculta: así la sesión (cookies en memoria) sigue viva mientras la app esté abierta.
    let w2 = win.clone();
    win.on_window_event(move |ev| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = ev { api.prevent_close(); let _ = w2.hide(); }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn u(s: &str) -> Url { Url::parse(s).unwrap() }
    #[test]
    fn navigation_rules() {
        assert!(nav_allowed(&u("https://educacionadistancia.juntadeandalucia.es/centros/cordoba/my/")));
        assert!(nav_allowed(&u("https://edea.juntadeandalucia.es/cas/login")));
        assert!(!nav_allowed(&u("https://evil.example/juntadeandalucia.es")));
        assert!(!nav_allowed(&u("http://edea.juntadeandalucia.es/")));
        assert!(!nav_allowed(&u("https://juntadeandalucia.es.evil.com/")));
        // el IdP no es "Moodle": no puede alimentar ingest ni moodle_navigate
        assert!(!is_allowed(&u("https://edea.juntadeandalucia.es/cas/login")));
    }
}

async fn remember_enabled(app: &AppHandle) -> bool {
    let st = app.state::<crate::state::AppState>();
    crate::db::get_setting(&st.pool, "remember_session").await.ok().flatten().as_deref() != Some("0")
}
