//! Linux: Moodle (y Familias) viven en un webview hijo de la ventana principal, no en una segunda ventana.
//!
//! WebKit no pinta ni ejecuta bien las páginas de Moodle si su widget no está mapeado (visibilityState «hidden»,
//! requestAnimationFrame parado, ancho 0), y una ventana extra que aparece y desaparece se nota. Aquí el webview
//! está siempre mapeado dentro de la ventana de AulaDesk, pero dentro de un contenedor GTK (un Layout, cuyo tamaño no depende del hijo) de 1×1 px, transparente y
//! con recorte, así que no se ve y la página sigue viendo un viewport normal (1100×800). Para iniciar sesión o terminar
//! una entrega a mano se pone a pantalla completa con una barra «Volver a AulaDesk».
use super::*;
use gtk::prelude::*;
use std::{cell::RefCell, rc::Rc};
use std::collections::HashMap;
use tauri::{LogicalPosition, LogicalSize, WebviewBuilder};

const HIDDEN_W: i32 = 1100;
const HIDDEN_H: i32 = 800;

#[derive(Clone)]
struct Ui {
    vbox: gtk::Box,
    app: gtk::Widget,
    bar: gtk::Box,
    fixed: gtk::Layout,
    moodle: gtk::Widget,
    shown: Rc<std::cell::Cell<bool>>,
}

impl Ui {
    fn apply(&self, shown: bool) {
        self.shown.set(shown);
        if shown {
            self.app.hide();
            self.bar.show();
            self.fixed.set_opacity(1.0);
            self.vbox.set_child_packing(&self.fixed, true, true, 0, gtk::PackType::Start);
            self.fixed.set_size_request(-1, -1);
            // Tamaño inicial: el de la ventana menos la barra; luego lo mantiene `connect_size_allocate`.
            let bar_h = self.bar.preferred_height().1;
            self.moodle.set_size_request(self.vbox.allocated_width().max(1), (self.vbox.allocated_height() - bar_h).max(1));
            self.moodle.grab_focus();
        } else {
            self.bar.hide();
            self.app.show();
            self.fixed.set_opacity(0.0);
            self.vbox.set_child_packing(&self.fixed, false, false, 0, gtk::PackType::Start);
            self.fixed.set_size_request(1, 1);
            self.moodle.set_size_request(HIDDEN_W, HIDDEN_H);
        }
    }
}

thread_local! { static UIS: RefCell<HashMap<String, Ui>> = RefCell::new(HashMap::new()); }

/// Muestra u oculta el webview `label`. Solo uno puede estar a pantalla completa: al mostrar uno, los demás se ocultan.
pub fn set_visible(app: &AppHandle, label: &str, visible: bool) {
    let label = label.to_string();
    let _ = app.run_on_main_thread(move || UIS.with(|u| {
        let map = u.borrow();
        if visible { for (l, ui) in map.iter() { if *l != label { ui.apply(false); } } }
        if let Some(ui) = map.get(&label) { ui.apply(visible); }
    }));
}

/// Reubica el webview de Moodle (ya creado como hijo) en el contenedor recortado. Se ejecuta en el hilo principal.
fn mount(label: &str, title: &str, moodle: gtk::Widget) -> Result<(), String> {
    let vbox = moodle.parent().and_then(|p| p.downcast::<gtk::Box>().ok()).ok_or("el webview no está en la caja de la ventana")?;
    // El webview de la aplicación es el único hijo que no es una barra (Box) ni un contenedor recortado (Layout) de otro webview montado antes.
    let app = vbox.children().into_iter().find(|c| *c != moodle && c.downcast_ref::<gtk::Box>().is_none() && c.downcast_ref::<gtk::Layout>().is_none())
        .ok_or("no se encontró el webview de la aplicación")?;
    vbox.remove(&moodle);

    let fixed = gtk::Layout::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    fixed.put(&moodle, 0, 0);
    moodle.set_size_request(HIDDEN_W, HIDDEN_H);

    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bar.set_margin(6);
    let back = gtk::Button::with_label("← Volver a AulaDesk");
    bar.pack_start(&back, false, false, 0);
    bar.pack_start(&gtk::Label::new(Some(title)), false, false, 6);
    for c in bar.children() { c.show_all(); }
    bar.set_no_show_all(true); // que un show_all() de la ventana no la muestre; apply() la enseña con show()

    vbox.pack_start(&bar, false, false, 0);
    vbox.pack_start(&fixed, false, false, 0);
    fixed.show_all();

    let ui = Ui { vbox, app, bar, fixed: fixed.clone(), moodle: moodle.clone(), shown: Rc::new(std::cell::Cell::new(false)) };
    // A pantalla completa el webview sigue el tamaño del contenedor.
    let (shown, mw) = (ui.shown.clone(), moodle.clone());
    fixed.connect_size_allocate(move |_, a| {
        if shown.get() && a.height() > 1 {
            let (w, h) = mw.size_request();
            if (w, h) != (a.width(), a.height()) { mw.set_size_request(a.width(), a.height()); }
        }
    });
    let ui2 = ui.clone();
    back.connect_clicked(move |_| ui2.apply(false));
    ui.apply(false);
    UIS.with(|u| { u.borrow_mut().insert(label.to_string(), ui); });
    Ok(())
}

/// Crea el webview como hijo de la ventana principal y lo monta oculto (1×1, transparente). Lo usan Moodle y Familias.
pub async fn attach(app: &AppHandle, builder: WebviewBuilder<tauri::Wry>, label: &str, title: &str) -> AppResult<Webview> {
    let main = app.get_window("main").ok_or_else(|| AppError::Other("no hay ventana principal".into()))?;
    let wv = main.add_child(builder, LogicalPosition::new(0.0, 0.0), LogicalSize::new(HIDDEN_W as f64, HIDDEN_H as f64))
        .map_err(|e| AppError::Other(format!("no se pudo abrir {title}: {e}")))?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    let (l, t) = (label.to_string(), title.to_string());
    wv.with_webview(move |pw| { let _ = tx.send(mount(&l, &t, pw.inner().upcast::<gtk::Widget>())); })
        .map_err(|e| AppError::Other(e.to_string()))?;
    rx.await.map_err(|e| AppError::Other(e.to_string()))?.map_err(AppError::Other)?;
    Ok(wv)
}

pub async fn open(app: &AppHandle, data_dir: &std::path::Path, visible: bool) -> AppResult<()> {
    if get(app).is_some() {
        if visible { set_visible(app, LABEL, true); }
        return Ok(());
    }
    let store = data_dir.join("webview");
    std::fs::create_dir_all(&store)?;
    let saved = saved_session(app).await;
    let initial = if saved.is_some() { Url::parse("about:blank").unwrap() } else { base_url() };
    let (emitter, dl_app) = (app.clone(), app.clone());
    let builder = WebviewBuilder::new(LABEL, WebviewUrl::External(initial))
        .data_directory(store)
        .initialization_script(BRIDGE)
        // Sin esto, Tauri captura los archivos soltados y la página de Moodle (selector de archivos de la entrega) nunca los recibe.
        .disable_drag_drop_handler()
        .on_navigation(handle_navigation)
        .on_download(move |_w, ev| handle_download(&dl_app, ev))
        .on_document_title_changed(|_w, t| handle_title(&t))
        .on_page_load(move |w, p| handle_load(&emitter, &w, &p));
    let wv = attach(app, builder, LABEL, "Moodle").await?;
    if let Some(cookies) = saved { restore_session(&wv, &cookies)?; }
    if visible { set_visible(app, LABEL, true); }
    tracing::info!("webview de Moodle creado dentro de la ventana principal");
    Ok(())
}

/// Ventana oculta extra de solo lectura (cursos y tareas en paralelo). Comparte el almacén de cookies con la principal.
pub async fn open_worker(app: &AppHandle, data_dir: &std::path::Path, i: usize) -> AppResult<Webview> {
    let label = worker_label(i);
    if let Some(w) = app.get_webview(&label) { return Ok(w); }
    let store = data_dir.join("webview");
    let emitter = app.clone();
    let builder = WebviewBuilder::new(&label, WebviewUrl::External(Url::parse("about:blank").unwrap()))
        .data_directory(store)
        .initialization_script(BRIDGE)
        .disable_drag_drop_handler()
        .on_navigation(handle_navigation)
        .on_download(|_w, _ev| false) // las descargas solo las gestiona la ventana principal
        .on_document_title_changed(|_w, t| handle_title(&t))
        .on_page_load(move |w, p| handle_load(&emitter, &w, &p));
    attach(app, builder, &label, "Moodle (lectura)").await
}

/// Cierra una ventana de lectura y retira su contenedor GTK.
pub fn close_worker(app: &AppHandle, i: usize) {
    let label = worker_label(i);
    if let Some(w) = app.get_webview(&label) { let _ = w.close(); }
    let _ = app.run_on_main_thread(move || UIS.with(|u| {
        if let Some(ui) = u.borrow_mut().remove(&label) { ui.vbox.remove(&ui.bar); ui.vbox.remove(&ui.fixed); }
    }));
}
