pub mod accounts;
mod audio;
pub mod calendar;
mod startup;
mod i18n;
pub mod familias;
pub mod integration;
pub mod commands;
pub mod dashboard;
pub mod autosync;
pub mod db;
pub mod devctl;
pub mod folder_backup;
pub mod scheduler;
pub mod downloads;
pub mod error;
pub mod files;
pub mod library;
pub mod notif;
pub mod notif_cmds;
pub mod notifier;
pub mod reminders;
pub mod moodle;
pub mod paths;
pub mod session_store;
pub mod state;
pub mod submission_cmds;
pub mod submissions;
pub mod tasks;
pub mod sync;

use tauri::Manager;

pub fn run() {
    let data_dir = std::env::var_os("AULADESK_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| paths::default_data_dir().expect("XDG"));
    paths::ensure_layout(&data_dir).expect("crear estructura de datos");

    let appender = tracing_appender::rolling::daily(data_dir.join("logs"), "auladesk.log");
    let (nb, _guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_writer(nb).with_ansi(false)
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    tracing::info!("AulaDesk {} arrancando", env!("CARGO_PKG_VERSION"));

    let pool = tauri::async_runtime::block_on(db::open_app(&data_dir.join("database/moodledesk.db")))
        .expect("abrir base de datos");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState { pool, data_dir, session: std::sync::Mutex::new(moodle::session::SessionState::Unknown),
            page_loads: tokio::sync::watch::channel(0u64).0, pending: std::sync::Mutex::new(None), worker_loads: std::array::from_fn(|_| tokio::sync::watch::channel(0u64).0), worker_pending: std::array::from_fn(|_| std::sync::Mutex::new(None)), dl: Default::default(), dl_name: Default::default(), dl_started: Default::default(), found_files: Default::default(), sync_running: Default::default(), sync_full: Default::default(), startup_claims_familias: Default::default(),
            familias_session: std::sync::Mutex::new(moodle::session::SessionState::Unknown), familias_page_loads: tokio::sync::watch::channel(0u64).0, familias_pending: Default::default(), familias_running: Default::default() })
        .on_window_event(|window, event| {
            // Ventana principal: con bandeja disponible, cerrar la oculta (la app sigue sincronizando); sin ella, cierra la app (la ventana de Moodle está solo oculta).
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    if autosync::TRAY_OK.load(std::sync::atomic::Ordering::SeqCst) && autosync::CLOSE_TO_TRAY.load(std::sync::atomic::Ordering::SeqCst) { api.prevent_close(); let _ = window.hide(); }
                    else { window.app_handle().exit(0); }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info, commands::get_setting, commands::set_setting, commands::backup_database,
            commands::moodle_open, commands::moodle_close, commands::moodle_navigate,
            commands::moodle_probe, autosync::autosync_status, commands::moodle_status, commands::moodle_account, commands::ingest_page,
            commands::moodle_forget_session, audio::audio_viz_start, audio::audio_viz_stop, startup::startup_sync_status, commands::moodle_sync, commands::moodle_sync_full, commands::list_courses, commands::course_tree,
            library::download_activity, library::download_course, library::course_cover, library::list_files, library::file_open,
            library::file_reveal, library::file_read, library::file_rename, library::file_move, library::folder_create, library::file_delete,
            library::list_duplicates, library::list_folders, library::import_file,
            notif_cmds::calendar_list, notif_cmds::calendar_create, notif_cmds::calendar_update, notif_cmds::calendar_delete, notif_cmds::calendar_set_notes, notif_cmds::notif_list, notif_cmds::notif_mark_read, notif_cmds::notif_unread_count, notif_cmds::notif_settings_get, notif_cmds::notif_settings_set, notif_cmds::notif_test, notif_cmds::service_status, notif_cmds::service_install, notif_cmds::service_uninstall,
            dashboard::dashboard_data,
            tasks::task_list, tasks::task_set_flags, tasks::note_set,
            submission_cmds::submission_prepare, submission_cmds::submission_history, submission_cmds::submission_cancel, submission_cmds::submission_start, submission_cmds::submission_submit, submission_cmds::submission_verify,
            scheduler::schedule_create, scheduler::schedule_list, scheduler::schedule_cancel, scheduler::pending_prompt_data, scheduler::schedule_mark_seen,
            folder_backup::folder_backup_status, folder_backup::folder_backup_set, folder_backup::folder_backup_submissions_set, folder_backup::folder_backup_run,
            integration::integrations_list, integration::integration_sync, integration::sync_all,
            familias::commands::ingest_familias, familias::commands::familias_open, familias::commands::familias_close, familias::commands::familias_status,
            familias::commands::familias_set_active_student, familias::commands::familias_sync, familias::commands::familias_recon, familias::commands::familias_pause,
            familias::commands::familias_disconnect, familias::commands::familias_grades, familias::commands::familias_attendance, familias::commands::familias_messages,
            familias::commands::familias_message_read, familias::commands::familias_overview,
            familias::commands::familias_action_prepare, familias::commands::familias_action_run, familias::commands::familias_action_cancel, familias::commands::familias_actions
        ])
        .setup(|app| { let _ = app.get_webview_window("main");
            // Sincronización automática + bandeja
            let h_auto = app.handle().clone();
            tauri::async_runtime::spawn(autosync::run_loop(h_auto));
            tauri::async_runtime::spawn(scheduler::run_loop(app.handle().clone()));
            tauri::async_runtime::spawn(familias::commands::run_loop(app.handle().clone()));
            tauri::async_runtime::spawn(familias::commands::keepalive_loop(app.handle().clone()));
            tauri::async_runtime::spawn(startup::run(app.handle().clone()));
            let h_tray = app.handle().clone();
            tauri::async_runtime::block_on(async { let st = h_tray.state::<state::AppState>();
                autosync::CLOSE_TO_TRAY.store(db::get_setting(&st.pool, "close_to_tray").await.ok().flatten().as_deref() != Some("0"), std::sync::atomic::Ordering::SeqCst); });
            autosync::build_tray(&h_tray);
            let dd = app.state::<state::AppState>().data_dir.clone();
            devctl::spawn(app.handle().clone(), dd.clone());
            // Avisos en segundo plano mientras la app está abierta (el servicio opcional hace lo mismo con la app cerrada; no se duplican).
            let pool_n = app.state::<state::AppState>().pool.clone();
            tauri::async_runtime::spawn(notifier::run_loop(pool_n, std::sync::Arc::new(notifier::NotifySend), std::time::Duration::from_secs(60)));
            // Sesión guardada: se abre la ventana de Moodle oculta y se restauran las cookies, sin que el usuario haga nada.
            let h = app.handle().clone();
            let dd_moodle = dd.clone();
            tauri::async_runtime::spawn(async move {
                let st = h.state::<state::AppState>();
                let on = db::get_setting(&st.pool, "remember_session").await.ok().flatten().as_deref() != Some("0");
                if on && tauri::async_runtime::spawn_blocking(|| session_store::load().is_some()).await.unwrap_or(false) {
                    if let Err(e) = moodle::window::open(&h, &dd_moodle, false).await { tracing::warn!("restaurar sesión: {e}"); }
                }
            });
            // Familias: igual, con su propio WebView y su propia entrada de llavero (nada de esto toca la sesión de Moodle).
            let (h2, dd2) = (app.handle().clone(), dd.clone());
            tauri::async_runtime::spawn(async move {
                let st = h2.state::<state::AppState>();
                let on = db::get_setting(&st.pool, "remember_session_familias").await.ok().flatten().as_deref() != Some("0");
                if on && tauri::async_runtime::spawn_blocking(|| session_store::load_for(session_store::FAMILIAS_USER).is_some()).await.unwrap_or(false) {
                    if let Err(e) = familias::window::open(&h2, &dd2, false).await { tracing::warn!("restaurar sesión de Familias: {e}"); }
                }
            });
            Ok(()) })
        .run(tauri::generate_context!())
        .expect("error al ejecutar AulaDesk");
}
