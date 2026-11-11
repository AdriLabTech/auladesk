//! Ventana de Familias: WebView propio y oculto con su almacén de cookies (`webview-familias`) y su entrada de llavero,
//! separados de los de Moodle aunque compartan dominio. El login lo hace el usuario a mano; aquí no se guardan ni leen contraseñas.
use crate::error::{AppError, AppResult};
use crate::session_store::{self, StoredCookie};
use tauri::webview::{DownloadEvent, PageLoadPayload};
use tauri::{AppHandle, Emitter, Manager, Url, Webview, WebviewUrl};

pub const LABEL: &str = "familias";
/// Portal de acceso (según el reconocimiento de 2026-11-09). Sin verificar tras el login.
pub const ENTRY: &str = "https://seneca.juntadeandalucia.es/seneca/jsp/portal/";
/// Con sesión viva, esta página entra directamente en Séneca (`/seneca/nav/*`); sin sesión muestra el formulario de acceso.
/// La página pública `portal/` enseña SIEMPRE el formulario, aunque haya sesión: no sirve para comprobarla.
pub const ACCESS: &str = "https://seneca.juntadeandalucia.es/seneca/jsp/Acceso.jsp";
pub const PORTAL_HOST: &str = "seneca.juntadeandalucia.es";
const BRIDGE: &str = include_str!("../../selectors/familias.js");
#[cfg(not(target_os = "linux"))]
const WINDOW_TITLE: &str = "Familias — AulaDesk";

pub fn entry_url() -> Url { Url::parse(ENTRY).expect("URL válida") }
pub fn access_url() -> Url { Url::parse(ACCESS).expect("URL válida") }

/// Navegación permitida: el portal y la pasarela de identidad de la Junta (https, *.juntadeandalucia.es). Nada más.
pub fn nav_allowed(u: &Url) -> bool {
    u.as_str() == "about:blank" || (u.scheme() == "https" && u.host_str().is_some_and(|h| h == "juntadeandalucia.es" || h.ends_with(".juntadeandalucia.es")))
}

/// Solo el portal puede enviar datos a `ingest_familias`.
pub fn is_portal(u: &Url) -> bool { u.scheme() == "https" && u.host_str() == Some(PORTAL_HOST) }

/// Las rutas del portal llevan tokens de sesión (`/seneca/nav/<token>`): en logs y mensajes solo se muestra su forma.
pub fn safe_path(path: &str) -> String {
    match path.strip_prefix("/seneca/nav/") { Some(_) => "/seneca/nav/*".into(), None => path.split('/').map(|p| if p.len() > 24 { "*" } else { p }).collect::<Vec<_>>().join("/") }
}

pub fn get(app: &AppHandle) -> Option<Webview> { app.get_webview(LABEL) }

pub fn set_visible(app: &AppHandle, visible: bool) {
    #[cfg(target_os = "linux")]
    crate::moodle::window::embedded::set_visible(app, LABEL, visible);
    #[cfg(not(target_os = "linux"))]
    if let Some(w) = app.get_webview_window(LABEL) { if visible { let _ = w.show(); let _ = w.set_focus(); } else { let _ = w.hide(); } }
}

async fn remember_enabled(app: &AppHandle) -> bool {
    let st = app.state::<crate::state::AppState>();
    crate::db::get_setting(&st.pool, "remember_session_familias").await.ok().flatten().as_deref() != Some("0")
}

async fn saved_session(app: &AppHandle) -> Option<Vec<StoredCookie>> {
    let saved = tauri::async_runtime::spawn_blocking(|| session_store::load_for(session_store::FAMILIAS_USER)).await.ok().flatten();
    if saved.is_some() && remember_enabled(app).await { saved } else { None }
}

fn restore_session(win: &Webview, cookies: &[StoredCookie]) -> AppResult<()> {
    for c in cookies { if let Err(e) = win.set_cookie(session_store::to_cookie(c)) { tracing::warn!("Familias: no se pudo restaurar una cookie: {e}"); } }
    tracing::info!("Familias: sesión guardada restaurada ({} cookies)", cookies.len());
    win.navigate(access_url()).map_err(|e| AppError::Other(e.to_string()))
}

fn handle_navigation(u: &Url) -> bool {
    let ok = nav_allowed(u);
    if !ok { tracing::warn!("Familias: navegación bloqueada fuera de la Junta: host={:?}", u.host_str()); }
    ok
}

/// Familias no descarga archivos por ahora: cualquier descarga se bloquea.
fn handle_download(ev: DownloadEvent<'_>) -> bool {
    if let DownloadEvent::Requested { url, .. } = ev { tracing::warn!("Familias: descarga bloqueada: host={:?}", url.host_str()); return false; }
    true
}

fn handle_load(app: &AppHandle, w: &Webview, p: &PageLoadPayload<'_>) {
    let u = p.url();
    tracing::info!("familias página {:?}: {}{}", p.event(), u.host_str().unwrap_or(""), safe_path(u.path()));
    if matches!(p.event(), tauri::webview::PageLoadEvent::Finished) {
        app.state::<crate::state::AppState>().familias_page_loads.send_modify(|n| *n += 1);
        if is_portal(u) { let _ = w.eval("window.__fmRun && window.__fmRun('session')"); }
        let _ = app.emit("familias-page", ());
    }
}

#[cfg(target_os = "linux")]
pub async fn open(app: &AppHandle, data_dir: &std::path::Path, visible: bool) -> AppResult<()> {
    use tauri::WebviewBuilder;
    if get(app).is_some() { if visible { set_visible(app, true); } return Ok(()); }
    let store = data_dir.join("webview-familias");
    std::fs::create_dir_all(&store)?;
    let saved = saved_session(app).await;
    let initial = if saved.is_some() { Url::parse("about:blank").unwrap() } else { access_url() };
    let emitter = app.clone();
    let builder = WebviewBuilder::new(LABEL, WebviewUrl::External(initial))
        .data_directory(store)
        .initialization_script(BRIDGE)
        .disable_drag_drop_handler()
        .on_navigation(handle_navigation)
        .on_download(|_w, ev| handle_download(ev))
        .on_page_load(move |w, p| handle_load(&emitter, &w, &p));
    let wv = crate::moodle::window::embedded::attach(app, builder, LABEL, "Familias").await?;
    if let Some(cookies) = saved { restore_session(&wv, &cookies)?; }
    if visible { set_visible(app, true); }
    tracing::info!("webview de Familias creado dentro de la ventana principal");
    Ok(())
}

/// Windows/macOS: ventana propia (sin verificar en ejecución, igual que la de Moodle).
#[cfg(not(target_os = "linux"))]
pub async fn open(app: &AppHandle, data_dir: &std::path::Path, visible: bool) -> AppResult<()> {
    use tauri::WebviewWindowBuilder;
    if let Some(w) = app.get_webview_window(LABEL) { if visible { let _ = w.show(); let _ = w.set_focus(); } return Ok(()); }
    let store = data_dir.join("webview-familias");
    std::fs::create_dir_all(&store)?;
    let saved = saved_session(app).await;
    let initial = if saved.is_some() { Url::parse("about:blank").unwrap() } else { access_url() };
    let emitter = app.clone();
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::External(initial))
        .title(WINDOW_TITLE).visible(visible).focused(false).inner_size(1100.0, 800.0)
        .data_directory(store).initialization_script(BRIDGE).disable_drag_drop_handler()
        .on_navigation(handle_navigation)
        .on_download(|_w, ev| handle_download(ev))
        .on_page_load(move |w, p| handle_load(&emitter, w.as_ref(), &p))
        .build().map_err(|e| AppError::Other(format!("no se pudo abrir Familias: {e}")))?;
    if let Some(cookies) = saved { restore_session(win.as_ref(), &cookies)?; }
    let w2 = win.clone();
    win.on_window_event(move |ev| { if let tauri::WindowEvent::CloseRequested { api, .. } = ev { api.prevent_close(); let _ = w2.hide(); } });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn u(s: &str) -> Url { Url::parse(s).unwrap() }
    #[test]
    fn navigation_is_limited_to_the_junta_over_https() {
        assert!(nav_allowed(&u("https://seneca.juntadeandalucia.es/seneca/jsp/portal/")));
        assert!(nav_allowed(&u("https://edea.juntadeandalucia.es/cas/login")));
        assert!(!nav_allowed(&u("http://seneca.juntadeandalucia.es/")));
        assert!(!nav_allowed(&u("https://juntadeandalucia.es.evil.com/")));
        assert!(!nav_allowed(&u("https://evil.example/seneca.juntadeandalucia.es")));
    }
    #[test]
    fn access_page_is_inside_the_allowed_portal() {
        assert!(nav_allowed(&access_url()) && is_portal(&access_url()));
    }
    #[test]
    fn only_the_portal_origin_can_ingest() {
        assert!(is_portal(&u("https://seneca.juntadeandalucia.es/seneca/nav/abc")));
        assert!(!is_portal(&u("https://edea.juntadeandalucia.es/cas/login")), "el IdP no puede alimentar datos");
        assert!(!is_portal(&u("https://educacionadistancia.juntadeandalucia.es/centros/cordoba/")), "Moodle tampoco");
        assert!(!is_portal(&u("http://seneca.juntadeandalucia.es/")));
    }
    #[test]
    fn tokens_never_reach_logs() {
        assert_eq!(safe_path("/seneca/nav/a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6"), "/seneca/nav/*");
        assert_eq!(safe_path("/seneca/jsp/portal/"), "/seneca/jsp/portal/");
        assert_eq!(safe_path("/x/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/y"), "/x/*/y");
    }
}
