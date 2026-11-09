//! Entrega de notificaciones al escritorio (Freedesktop) y bucle en segundo plano. Funciona solo con la BD:
//! lo ejecutan tanto la app abierta como el modo `--notifier` (servicio de usuario), sin duplicar avisos.
use crate::notif::NotifRow;
use crate::reminders::{self, allowed_now, NotifSettings};
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Duration;

pub trait Sink: Send + Sync {
    fn show(&self, n: &NotifRow) -> Result<(), String>;
}

/// Notificación estándar de Freedesktop mediante `notify-send` (libnotify). Funciona con cualquier demonio (mako, dunst, swaync, Quickshell…).
pub struct NotifySend;
impl Sink for NotifySend {
    fn show(&self, n: &NotifRow) -> Result<(), String> {
        let urgency = match n.priority { 2.. => "critical", 1 => "normal", _ => "low" };
        let status = std::process::Command::new("notify-send")
            .args(["-a", "AulaDesk", "-u", urgency, "-c", &format!("auladesk.{}", n.category), "-i", "appointment-soon", "--", &n.title, n.body.as_deref().unwrap_or("")])
            .status().map_err(|e| format!("notify-send no disponible: {e}"))?;
        if status.success() { Ok(()) } else { Err(format!("notify-send terminó con {status}")) }
    }
}

/// Muestra en el escritorio las notificaciones pendientes. En silencio no se muestra nada (quedan en la bandeja);
/// durante la franja silenciosa o un día no permitido se aplazan.
pub async fn deliver(pool: &SqlitePool, sink: &dyn Sink, now: i64, s: &NotifSettings) -> usize {
    if s.muted {
        let _ = sqlx::query("UPDATE notification SET shown_at=? WHERE shown_at IS NULL").bind(now).execute(pool).await;
        return 0;
    }
    if !allowed_now(s, now) { return 0; }
    let rows: Vec<NotifRow> = sqlx::query_as("SELECT id,category,priority,title,body,ref_kind,ref_id,created_at,read_at,shown_at FROM notification WHERE shown_at IS NULL ORDER BY id LIMIT 20")
        .fetch_all(pool).await.unwrap_or_default();
    let mut shown = 0;
    for n in rows {
        let claimed = sqlx::query("UPDATE notification SET shown_at=? WHERE id=? AND shown_at IS NULL").bind(now).bind(n.id).execute(pool).await.map(|r| r.rows_affected()).unwrap_or(0);
        if claimed != 1 { continue; }   // otro proceso ya la ha mostrado
        match sink.show(&n) { Ok(()) => shown += 1, Err(e) => tracing::warn!("notificación de escritorio: {e}") }
    }
    shown
}

/// Un ciclo completo: poner al día avisos, disparar los vencidos, resúmenes y mostrar lo pendiente.
pub async fn tick(pool: &SqlitePool, sink: &dyn Sink, now: i64) -> usize {
    let s = reminders::load_settings(pool).await;
    if let Err(e) = reminders::reconcile(pool, now, &s).await { tracing::warn!("reconciliar avisos: {e}"); }
    if let Err(e) = reminders::fire_due(pool, now, &s).await { tracing::warn!("disparar avisos: {e}"); }
    if let Err(e) = reminders::run_digests(pool, now, &s).await { tracing::warn!("resúmenes: {e}"); }
    deliver(pool, sink, now, &s).await
}

pub async fn run_loop(pool: SqlitePool, sink: Arc<dyn Sink>, every: Duration) {
    loop {
        tick(&pool, sink.as_ref(), chrono::Utc::now().timestamp()).await;
        tokio::time::sleep(every).await;
    }
}

/// Modo sin ventana (`auladesk --notifier`): solo lee la BD, nunca toca Moodle.
pub fn run_headless() {
    let data_dir = std::env::var_os("AULADESK_DATA_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| crate::paths::default_data_dir().expect("XDG"));
    crate::paths::ensure_layout(&data_dir).expect("estructura de datos");
    let appender = tracing_appender::rolling::daily(data_dir.join("logs"), "notifier.log");
    let (nb, _guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt().with_writer(nb).with_ansi(false).with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into())).init();
    tracing::info!("notificador en segundo plano arrancando");
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().worker_threads(1).build().expect("runtime");
    rt.block_on(async {
        let pool = crate::db::open_app(&data_dir.join("database/moodledesk.db")).await.expect("abrir BD");
        run_loop(pool, Arc::new(NotifySend), Duration::from_secs(60)).await;
    });
}

// ---------------- servicio de usuario (systemd --user) ----------------
const UNIT: &str = "auladesk-notifier.service";

#[derive(serde::Serialize)]
pub struct ServiceStatus { pub installed: bool, pub active: bool, pub unit_path: String }

fn unit_path() -> std::path::PathBuf {
    directories::BaseDirs::new().map(|b| b.config_dir().join("systemd/user").join(UNIT)).unwrap_or_else(|| std::path::PathBuf::from(UNIT))
}

pub fn service_status() -> ServiceStatus {
    let p = unit_path();
    let active = std::process::Command::new("systemctl").args(["--user", "is-active", "--quiet", UNIT]).status().map(|s| s.success()).unwrap_or(false);
    ServiceStatus { installed: p.exists(), active, unit_path: p.display().to_string() }
}

pub fn unit_text(exe: &str, data_dir: Option<&str>) -> String {
    let env = data_dir.map(|d| format!("Environment=AULADESK_DATA_DIR={d}\n")).unwrap_or_default();
    format!("[Unit]\nDescription=AulaDesk: avisos de entregas y exámenes (sin ventana)\nAfter=graphical-session.target\n\n[Service]\nExecStart={exe} --notifier\n{env}Restart=on-failure\nRestartSec=15\nNice=10\n\n[Install]\nWantedBy=default.target\n")
}

pub fn service_install() -> Result<ServiceStatus, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let p = unit_path();
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&p, unit_text(&exe.display().to_string(), std::env::var("AULADESK_DATA_DIR").ok().as_deref())).map_err(|e| e.to_string())?;
    for args in [vec!["--user", "daemon-reload"], vec!["--user", "enable", "--now", UNIT]] {
        let out = std::process::Command::new("systemctl").args(&args).output().map_err(|e| e.to_string())?;
        if !out.status.success() { return Err(format!("systemctl {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim())); }
    }
    Ok(service_status())
}

pub fn service_uninstall() -> Result<ServiceStatus, String> {
    let _ = std::process::Command::new("systemctl").args(["--user", "disable", "--now", UNIT]).output();
    let p = unit_path();
    if p.exists() { std::fs::remove_file(&p).map_err(|e| e.to_string())?; }
    let _ = std::process::Command::new("systemctl").args(["--user", "daemon-reload"]).output();
    Ok(service_status())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notif;
    use std::sync::Mutex;
    struct Rec(Mutex<Vec<String>>);
    impl Sink for Rec { fn show(&self, n: &NotifRow) -> Result<(), String> { self.0.lock().unwrap().push(n.title.clone()); Ok(()) } }

    #[tokio::test]
    async fn delivers_once_respects_mute_and_quiet_hours() {
        let d = tempfile::tempdir().unwrap();
        let p = crate::db::open(&d.path().join("t.db")).await.unwrap();
        let day = chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2026, 11, 10, 9, 0, 0).unwrap().timestamp(); // 10:00 Madrid
        notif::add(&p, "deadline", 1, "Entrega mañana: X", Some("c"), None, None).await.unwrap();
        let rec = Rec(Mutex::new(vec![]));
        let s = NotifSettings::default();
        let night = chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2026, 11, 10, 22, 30, 0).unwrap().timestamp();
        assert_eq!(deliver(&p, &rec, night, &s).await, 0, "franja silenciosa: se aplaza");
        assert_eq!(deliver(&p, &rec, day, &s).await, 1);
        assert_eq!(deliver(&p, &rec, day + 60, &s).await, 0, "solo se muestra una vez");
        assert_eq!(rec.0.lock().unwrap().len(), 1);
        // en silencio: queda en la bandeja, no sale al escritorio, y al reactivar no llega un aluvión
        notif::add(&p, "deadline", 1, "Otra", None, None, None).await.unwrap();
        let muted = NotifSettings { muted: true, ..Default::default() };
        assert_eq!(deliver(&p, &rec, day + 120, &muted).await, 0);
        assert_eq!(deliver(&p, &rec, day + 180, &s).await, 0);
        assert_eq!(notif::list(&p, 10, false).await.unwrap().len(), 2, "siguen en la bandeja");
        assert_eq!(notif::unread_count(&p).await.unwrap(), 2);
        notif::mark_read(&p, None).await.unwrap();
        assert_eq!(notif::unread_count(&p).await.unwrap(), 0);
    }

    #[test]
    fn unit_file_is_well_formed() {
        let u = unit_text("/home/x/auladesk", Some("/tmp/d"));
        assert!(u.contains("ExecStart=/home/x/auladesk --notifier\n") && u.contains("Environment=AULADESK_DATA_DIR=/tmp/d\n") && u.contains("WantedBy=default.target"));
        assert!(!unit_text("/x", None).contains("Environment="));
    }
}
