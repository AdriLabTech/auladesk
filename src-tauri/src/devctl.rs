//! Canal de control SOLO PARA DESARROLLO (activo con AULADESK_DEV_CTL=1). Lee órdenes de cache/ctl.txt:
//!   nav <ruta relativa a la base de Moodle> | probe <dom|session> | wait <segundos>
//! Aplica las mismas validaciones que los comandos normales. Nunca se activa en uso normal.
use crate::moodle::window;
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub fn spawn(app: AppHandle, data_dir: PathBuf) {
    if std::env::var("AULADESK_DEV_CTL").as_deref() != Ok("1") { return; }
    tracing::warn!("canal de desarrollo ACTIVO (AULADESK_DEV_CTL=1)");
    tauri::async_runtime::spawn(async move {
        let ctl = data_dir.join("cache/ctl.txt");
        loop {
            tokio::time::sleep(Duration::from_millis(700)).await;
            let Ok(text) = tokio::fs::read_to_string(&ctl).await else { continue };
            if text.trim().is_empty() { continue; }
            let _ = tokio::fs::write(&ctl, "").await;
            for line in text.lines() {
                let mut it = line.trim().splitn(2, ' ');
                let (cmd, arg) = (it.next().unwrap_or(""), it.next().unwrap_or("").trim());
                let w = window::get(&app);
                match (cmd, w) {
                    ("open", _) => { let dd = data_dir.clone(); if let Err(e) = window::open(&app, &dd, true).await { tracing::warn!("devctl open: {e}"); } }
                    ("fetch", _) => {
                        let st = app.state::<crate::state::AppState>();
                        match crate::library::fetch_url(&app, &st, arg).await { Ok(s) => tracing::info!("devctl fetch: {} sha={}", s.rel_path, &s.sha256[..16]), Err(e) => tracing::warn!("devctl fetch: {e}") }
                    }
                    ("prepare", _) => {
                        let st = app.state::<crate::state::AppState>();
                        let (id, path) = arg.split_once(' ').unwrap_or(("0", ""));
                        match crate::submissions::prepare(&st.pool, &st.data_dir.join("files"), id.parse().unwrap_or(0), std::path::Path::new(path)).await {
                            Ok(v) => tracing::info!("devctl prepare: versión {} (v{}) sha={}", v.id, v.version, &v.sha256[..16]),
                            Err(e) => tracing::warn!("devctl prepare: {e}") }
                    }
                    ("notiftest", _) => {
                        let st = app.state::<crate::state::AppState>();
                        match crate::notif_cmds::notif_test(st).await { Ok(()) => tracing::info!("devctl notiftest: enviada"), Err(e) => tracing::warn!("devctl notiftest: {e}") }
                    }
                    ("start", _) => {
                        // Equivale a pulsar «Entregar en Moodle…» tras la confirmación explícita del usuario (dada en el chat).
                        match crate::submission_cmds::submission_start(app.clone(), arg.parse().unwrap_or(0), true).await {
                            Ok(()) => tracing::info!("devctl start: página de entrega abierta"), Err(e) => tracing::warn!("devctl start: {e}") }
                    }
                    ("verify", _) => {
                        match crate::submission_cmds::submission_verify(app.clone(), arg.parse().unwrap_or(0)).await {
                            Ok(v) => tracing::info!("devctl verify: versión {} → {} | {}", v.id, v.state, v.confirmation_evidence.unwrap_or_default()),
                            Err(e) => tracing::warn!("devctl verify: {e}") }
                    }
                    ("hide", _) => { window::set_visible(&app, false); }
                    ("wait", _) => tokio::time::sleep(Duration::from_secs(arg.parse().unwrap_or(1))).await,
                    ("nav", Some(w)) => match window::base_url().join(arg) {
                        Ok(u) if window::is_allowed(&u) => { let _ = w.navigate(u); tracing::info!("devctl nav {}", arg.split('?').next().unwrap_or("")); }
                        _ => tracing::warn!("devctl: ruta rechazada"),
                    },
                    ("assign", Some(w)) => {
                        // Sondeo puntual de una tarea (por cmid) para pruebas.
                        let _ = w.eval("1");
                        let st = app.state::<crate::state::AppState>();
                        match crate::commands::step_pub(&app, &st, &format!("mod/assign/view.php?id={arg}"), "assignment").await { Ok(s) => tracing::info!("devctl assign {arg}: ok cambios={}", s.changed), Err(e) => tracing::warn!("devctl assign {arg}: {e}") }
                    }
                    ("download", _) => {
                        let id: i64 = arg.parse().unwrap_or(0);
                        match crate::library::download_activity(app.clone(), id).await {
                            Ok(r) => tracing::info!("devctl download: {} guardados, {} errores", r.stored.len(), r.errors.len()),
                            Err(e) => tracing::warn!("devctl download falló: {e}"),
                        }
                    }
                    ("sync", _) => {
                        match crate::commands::run_sync(app.clone()).await { Ok(r) => tracing::info!("devctl sync: {} cursos, {} ok, {} fallos", r.courses, r.synced, r.failed.len()), Err(e) => tracing::warn!("devctl sync falló: {e}") }
                    }
                    ("probe", Some(w)) if crate::commands::PROBES.contains(&arg) => {
                        let _ = w.eval(format!("window.__mdRun && window.__mdRun('{arg}')"));
                    }
                    _ => tracing::warn!("devctl: orden ignorada: {cmd}"),
                }
            }
        }
    });
}
