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
                    // ── Familias (solo desarrollo) ──
                    ("fopen", _) => { let dd = data_dir.clone(); if let Err(e) = crate::familias::window::open(&app, &dd, true).await { tracing::warn!("devctl fopen: {e}"); } }
                    ("fhide", _) => { crate::familias::window::set_visible(&app, false); }
                    ("frecon", _) => { match crate::familias::commands::familias_recon(app.clone()).await { Ok(()) => tracing::info!("devctl frecon: pedido"), Err(e) => tracing::warn!("devctl frecon: {e}") } }
                    ("fsync", _) => { match crate::familias::commands::run_sync(app.clone(), true).await { Ok(r) => tracing::info!("devctl fsync: {} recetas, {} elementos, {} fallos", r.recipes, r.seen, r.failed.len()), Err(e) => tracing::warn!("devctl fsync: {e}") } }
                    ("fnav", _) => {
                        if let (Some(w), Ok(u)) = (crate::familias::window::get(&app), crate::familias::window::entry_url().join(arg)) {
                            if crate::familias::window::is_portal(&u) { let _ = w.navigate(u); tracing::info!("devctl fnav {}", crate::familias::window::safe_path(arg)); } else { tracing::warn!("devctl fnav: ruta rechazada"); }
                        }
                    }
                    // feval <fichero.js>: ejecuta un script de LECTURA en el portal (solo desarrollo; el script solo puede llamar a ingest_familias).
                    ("feval", _) => { if let (Some(w), Ok(js)) = (crate::familias::window::get(&app), tokio::fs::read_to_string(arg).await) { let _ = w.eval(js); tracing::info!("devctl feval"); } else { tracing::warn!("devctl feval: sin ventana o fichero ilegible"); } }
                    // fcounts: recuentos de filas de Familias (sin contenido) en el log, para verificar la sincronización.
                    // syncall: sincroniza Moodle y Familias a la vez y registra el resultado y el tiempo de cada una.
                    ("syncall", _) => {
                        let t = std::time::Instant::now();
                        match crate::integration::sync_all(app.clone()).await {
                            Ok(r) => { for o in &r { tracing::info!("devctl syncall: {} ok={} {} ms · {}", o.platform, o.ok, o.millis, o.message.chars().take(160).collect::<String>()); } tracing::info!("devctl syncall: total {} ms", t.elapsed().as_millis()); }
                            Err(e) => tracing::warn!("devctl syncall: {e}"),
                        }
                    }
                    ("fcounts", _) => {
                        let st = app.state::<crate::state::AppState>();
                        let mut out = vec![];
                        for (name, sql) in [("alumnos", "SELECT count(*) FROM student WHERE removed_at IS NULL"), ("notas", "SELECT count(*) FROM grade_record WHERE removed_at IS NULL"), ("faltas", "SELECT count(*) FROM attendance_record WHERE removed_at IS NULL"), ("faltas_injustificadas", "SELECT count(*) FROM attendance_record WHERE removed_at IS NULL AND justified=0 AND kind='falta'"), ("retrasos", "SELECT count(*) FROM attendance_record WHERE removed_at IS NULL AND kind='retraso'"), ("mensajes", "SELECT count(*) FROM school_message WHERE removed_at IS NULL"), ("mensajes_sin_leer", "SELECT count(*) FROM school_message WHERE removed_at IS NULL AND read_in_portal=0"), ("avisos_familias", "SELECT count(*) FROM notification WHERE source='familias'")] {
                            let n: i64 = sqlx::query_scalar(sql).fetch_one(&st.pool).await.unwrap_or(-1);
                            out.push(format!("{name}={n}"));
                        }
                        tracing::info!("devctl fcounts: {}", out.join(" "));
                    }
                    ("fclick", _) => { if let Some(w) = crate::familias::window::get(&app) { let a = serde_json::to_string(&serde_json::json!({ "text": arg }).to_string()).unwrap(); let _ = w.eval(format!("window.__fmRun && window.__fmRun('click', {a})")); } }
                    // fingest <tipo> <fichero.json>: aplica un payload simulado por el MISMO camino de validación y persistencia que el portal.
                    ("fingest", _) => {
                        let (kind, file) = arg.split_once(' ').unwrap_or(("", ""));
                        let st = app.state::<crate::state::AppState>();
                        let acc = crate::accounts::ensure(&st.pool, "familias").await.unwrap_or(0);
                        match tokio::fs::read_to_string(file).await.ok().and_then(|t| serde_json::from_str(&t).ok()) {
                            Some(v) => match crate::familias::commands::apply(&st.pool, acc, kind, v).await { Ok(s) => tracing::info!("devctl fingest {kind}: {} vistos, {} cambios", s.seen, s.changed), Err(e) => tracing::warn!("devctl fingest {kind}: {e}") },
                            None => tracing::warn!("devctl fingest: fichero ilegible"),
                        }
                    }
                    ("meval", Some(w)) => { if let Ok(js) = tokio::fs::read_to_string(arg).await { let _ = w.eval(js); tracing::info!("devctl meval: {arg}"); } }
        ("syncfull", _) => { match crate::commands::moodle_sync_full(app.clone()).await { Ok(r) => tracing::info!("devctl syncfull: {} cursos, {} tareas, {} fallos", r.courses, r.assignments, r.failed.len()), Err(e) => tracing::warn!("devctl syncfull: {e}") } }
        ("audio", _) => {
            use tauri::Listener;
            let frames = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Vec<u8>>::new()));
            let f2 = frames.clone();
            let id = app.listen("audio-levels", move |e| { if let Ok(v) = serde_json::from_str::<Vec<u8>>(e.payload()) { f2.lock().unwrap().push(v); } });
            match crate::audio::audio_viz_start(app.clone()) { Ok(()) => tracing::info!("devctl audio: captura iniciada"), Err(e) => tracing::warn!("devctl audio: {e}") }
            tokio::time::sleep(Duration::from_secs(arg.parse().unwrap_or(8))).await;
            crate::audio::audio_viz_stop(); app.unlisten(id);
            let fr = frames.lock().unwrap();
            let peak = fr.iter().map(|f| *f.iter().max().unwrap_or(&0)).max().unwrap_or(0);
            let quiet = fr.iter().filter(|f| f.iter().all(|v| *v == 0)).count();
            let hot = fr.iter().max_by_key(|f| *f.iter().max().unwrap_or(&0)).map(|f| f.iter().enumerate().max_by_key(|(_, v)| **v).map(|(i, _)| i).unwrap_or(0));
            for (i, f) in fr.iter().enumerate().filter(|(i, _)| i % 24 == 0) { tracing::info!("devctl audio t≈{:.1}s {:?}", i as f32 / 47.0, f); }
            tracing::info!("devctl audio: {} fotogramas, {} en silencio, pico {} (banda {:?}), muestra {:?}", fr.len(), quiet, peak, hot, fr.iter().max_by_key(|f| *f.iter().max().unwrap_or(&0)));
        }
        ("fcookies", _) => {
            // Solo metadatos (nombre, dominio, caducidad): nunca valores.
            match tauri::async_runtime::spawn_blocking(|| crate::session_store::load_for(crate::session_store::FAMILIAS_USER)).await.ok().flatten() {
                Some(v) => for c in v { tracing::info!("devctl fcookies: {} dominio={:?} ruta={:?} caduca={:?} secure={} httponly={} valor_len={}", c.name, c.domain, c.path, c.expires.map(|t| t - chrono::Utc::now().timestamp()), c.secure, c.http_only, c.value.len()); },
                None => tracing::info!("devctl fcookies: sin cookies guardadas"),
            }
        }
        ("setting", _) => { if let Some((k, v)) = arg.split_once(' ') { match crate::db::set_setting(&app.state::<crate::state::AppState>().pool, k, v.trim()).await { Ok(()) => tracing::info!("devctl setting {k}={}", v.trim()), Err(e) => tracing::warn!("devctl setting: {e}") } } }
        ("player", _) => { let r = crate::player::player_state().await; tracing::info!("devctl player: {} | {} | «{}» de {} | {:.0}/{:.0} s | disponible={}", r.player, r.status, r.title, r.artist, r.position, r.length, r.available); }
        ("playerpp", _) => {
            let a = crate::player::player_control("play_pause".into(), None).await; tracing::info!("devctl playerpp 1: {:?}", a.map(|s| s.status));
            tokio::time::sleep(Duration::from_millis(700)).await;
            let b = crate::player::player_control("play_pause".into(), None).await; tracing::info!("devctl playerpp 2: {:?}", b.map(|s| s.status));
        }
        ("hide", _) => { window::set_visible(&app, false); }
                    ("wait", _) => tokio::time::sleep(Duration::from_secs(arg.parse().unwrap_or(1))).await,
                    ("nav", Some(w)) => match window::base_url().join(arg) {
                        Ok(u) if window::is_allowed(&u) => { let _ = w.navigate(u); tracing::info!("devctl nav {}", arg.split('?').next().unwrap_or("")); }
                        _ => tracing::warn!("devctl: ruta rechazada"),
                    },
                    // SPIKE PASEN (solo lectura): navega a una URL https de *.juntadeandalucia.es para verla con la sesión del propio usuario.
                    ("navext", Some(w)) => match arg.parse::<tauri::Url>() {
                        Ok(u) if window::nav_allowed(&u) && u.scheme() == "https" => { let _ = w.navigate(u.clone()); tracing::info!("devctl navext {}", u.host_str().unwrap_or("")); }
                        _ => tracing::warn!("devctl: navext rechazado"),
                    },
                    // SPIKE: pulsa el primer enlace/botón cuyo texto contiene `arg` (solo en la ventana ya cargada).
                    ("clicktext", Some(w)) => {
                        let t = serde_json::to_string(arg).unwrap_or_default();
                        let _ = w.eval(format!("(()=>{{const t={t};const e=[...document.querySelectorAll('a,button')].find(x=>x.textContent.includes(t));if(e)e.click();}})()"));
                        tracing::info!("devctl clicktext");
                    }
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
