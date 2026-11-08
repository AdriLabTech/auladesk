//! Ventana de Moodle: WebView con almacén de cookies propio y persistente. Aquí no se leen ni copian cookies.
use crate::error::{AppError, AppResult};
use tauri::{AppHandle, Emitter, Manager, Url, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "moodle";
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

pub async fn open(app: &AppHandle, data_dir: &std::path::Path, visible: bool) -> AppResult<()> {
    if let Some(w) = app.get_webview_window(LABEL) {
        if !visible { return Ok(()); }
        w.show().map_err(|e| AppError::Other(e.to_string()))?;
        w.set_focus().ok();
        return Ok(());
    }
    let store = data_dir.join("webview");
    std::fs::create_dir_all(&store)?;
    // Con sesión guardada se carga about:blank, se restauran las cookies y entonces se navega a Moodle.
    let saved = if tauri::async_runtime::spawn_blocking(crate::session_store::load).await.ok().flatten().is_some() && remember_enabled(app).await {
        tauri::async_runtime::spawn_blocking(crate::session_store::load).await.ok().flatten()
    } else { None };
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
        .on_navigation(|u| {
            let ok = nav_allowed(u);
            if !ok { tracing::warn!("navegación bloqueada fuera de Moodle: host={:?}", u.host_str()); }
            ok
        })
        .on_download(move |_w, ev| {
            use tauri::webview::DownloadEvent;
            match ev {
                DownloadEvent::Requested { url, destination } => {
                    if !nav_allowed(&url) { tracing::warn!("descarga bloqueada fuera de Moodle: host={:?}", url.host_str()); return false; }
                    let st = dl_app.state::<crate::state::AppState>();
                    *st.dl_name.lock().unwrap() = destination.file_name().map(|n| n.to_string_lossy().to_string());
                    st.dl_started.store(true, std::sync::atomic::Ordering::SeqCst);
                    let dir = st.data_dir.join("cache/downloads");
                    let _ = std::fs::create_dir_all(&dir);
                    *destination = dir.join(format!("{}.part", uuid::Uuid::new_v4()));
                    tracing::info!("descarga iniciada: {}{}", url.host_str().unwrap_or(""), url.path());
                    true
                }
                DownloadEvent::Finished { url, path, success } => {
                    let app = dl_app.clone();
                    tauri::async_runtime::spawn(crate::library::on_download_finished(app, url.to_string(), path, success));
                    true
                }
                _ => true,
            }
        })
        .on_document_title_changed(|_w, t| {
            if t.starts_with("[mdbridge error") { tracing::warn!("puente: {}", t.split(']').next().unwrap_or("")); }
        })
        .on_page_load(move |w, p| {
            // Solo host+ruta: nunca query ni fragmento (pueden llevar tokens).
            let u = p.url();
            tracing::info!("moodle página {:?}: {}{}", p.event(), u.host_str().unwrap_or(""), u.path());
            let _ = emitter.emit("moodle-page", u.path().to_string());
            if matches!(p.event(), tauri::webview::PageLoadEvent::Finished) {
                emitter.state::<crate::state::AppState>().page_loads.send_modify(|n| *n += 1);
            }
            if matches!(p.event(), tauri::webview::PageLoadEvent::Finished) && is_allowed(u) {
                let _ = w.eval("window.__mdRun && window.__mdRun('session')");
            }
        })
        .build()
        .map_err(|e| AppError::Other(format!("no se pudo abrir Moodle: {e}")))?;
    if let Some(cookies) = saved {
        let n = cookies.len();
        for c in &cookies {
            if let Err(e) = win.set_cookie(crate::session_store::to_cookie(c)) { tracing::warn!("no se pudo restaurar una cookie: {e}"); }
        }
        tracing::info!("sesión guardada restaurada ({n} cookies); comprobando con Moodle");
        win.navigate(base_url().join("my/").map_err(|e| AppError::Other(e.to_string()))?).map_err(|e| AppError::Other(e.to_string()))?;
    }
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
