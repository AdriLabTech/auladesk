//! Recordatorios y avisos por reglas (sin IA): 7/3/1 días antes de una fecha límite, cancelados al entregar,
//! actualizados si la fecha cambia, respetando horario, días permitidos y categorías. Todo se decide desde la BD:
//! funciona aunque Moodle no esté disponible y sin ventana abierta.
use crate::db;
use crate::error::AppResult;
use crate::moodle::dates::DEFAULT_TZ as TZ;
use crate::notif;
use crate::tasks::derive_state;
use chrono::{Datelike, TimeZone, Timelike};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NotifSettings {
    /// Días de antelación de los avisos de fecha límite/examen.
    pub lead_days: Vec<i64>,
    /// Franja SILENCIOSA (no se avisa dentro de ella); si empieza y acaba igual, no hay franja.
    pub quiet_start: String, pub quiet_end: String,
    /// Lunes..domingo: días en los que se puede avisar.
    pub weekdays: [bool; 7],
    pub muted: bool,
    pub daily_digest: Option<String>,
    pub weekly_digest: Option<String>, pub weekly_day: u8,
    /// Categorías: deadline, exam, reminder, new_activity, date_changed, grade, submission, sync_error, download, digest.
    pub categories: HashMap<String, bool>,
}

impl Default for NotifSettings {
    fn default() -> Self {
        let mut categories: HashMap<String, bool> = ["deadline", "exam", "reminder", "new_activity", "date_changed", "grade", "submission", "sync_error", "digest"].iter().map(|c| (c.to_string(), true)).collect();
        categories.insert("download".into(), false); // las descargas las inicia el usuario: no hace falta avisar
        NotifSettings { lead_days: vec![7, 3, 1], quiet_start: "22:00".into(), quiet_end: "08:00".into(), weekdays: [true; 7], muted: false, daily_digest: None, weekly_digest: None, weekly_day: 0, categories }
    }
}

impl NotifSettings {
    pub fn enabled(&self, category: &str) -> bool { self.categories.get(category).copied().unwrap_or(true) }
    fn validated(mut self) -> Self {
        self.lead_days.retain(|d| (0..=60).contains(d));
        self.lead_days.sort_unstable_by(|a, b| b.cmp(a)); self.lead_days.dedup();
        self.weekly_day = self.weekly_day.min(6);
        self
    }
}

pub async fn load_settings(pool: &SqlitePool) -> NotifSettings {
    db::get_setting(pool, "notifications").await.ok().flatten().and_then(|j| serde_json::from_str::<NotifSettings>(&j).ok()).unwrap_or_default().validated()
}

pub async fn save_settings(pool: &SqlitePool, s: &NotifSettings) -> AppResult<NotifSettings> {
    let s = s.clone().validated();
    for t in [Some(&s.quiet_start), Some(&s.quiet_end), s.daily_digest.as_ref(), s.weekly_digest.as_ref()].into_iter().flatten() {
        if minutes(t).is_none() { return Err(crate::error::AppError::Other(format!("Hora no válida: «{t}» (usa HH:MM)"))); }
    }
    db::set_setting(pool, "notifications", &serde_json::to_string(&s).unwrap()).await?;
    Ok(s)
}

fn minutes(hhmm: &str) -> Option<i64> {
    let (h, m) = hhmm.split_once(':')?;
    let (h, m): (i64, i64) = (h.trim().parse().ok()?, m.trim().parse().ok()?);
    ((0..24).contains(&h) && (0..60).contains(&m)).then_some(h * 60 + m)
}

fn local(ts: i64) -> chrono::DateTime<chrono_tz::Tz> { TZ.timestamp_opt(ts, 0).single().unwrap_or_else(|| TZ.timestamp_opt(0, 0).unwrap()) }
fn day_index(ts: i64) -> i64 { local(ts).date_naive().num_days_from_ce() as i64 }

/// ¿Se puede avisar ahora? Día permitido y fuera de la franja silenciosa (hora de Madrid).
pub fn allowed_now(s: &NotifSettings, now: i64) -> bool {
    let l = local(now);
    if !s.weekdays[l.weekday().num_days_from_monday() as usize] { return false; }
    let m = (l.hour() * 60 + l.minute()) as i64;
    let (qs, qe) = (minutes(&s.quiet_start).unwrap_or(0), minutes(&s.quiet_end).unwrap_or(0));
    if qs == qe { true } else if qs < qe { !(m >= qs && m < qe) } else { !(m >= qs || m < qe) }
}

#[derive(Clone, Copy)]
enum Owner { Activity(i64), Event(i64) }

async fn ensure(pool: &SqlitePool, owner: Owner, rule: &str, fire_at: i64, target_at: i64, now: i64, catchup: bool) -> AppResult<()> {
    let (col, id) = match owner { Owner::Activity(i) => ("activity_id", i), Owner::Event(i) => ("event_id", i) };
    let row: Option<(i64, Option<i64>, Option<i64>, Option<i64>)> = sqlx::query_as(&format!("SELECT id, target_at, fired_at, cancelled_at FROM reminder WHERE {col}=? AND rule=?")).bind(id).bind(rule).fetch_optional(pool).await?;
    match row {
        None => {
            let (fire, fired, title) = if fire_at > now { (fire_at, None, None) } else if catchup { (now, None, None) } else { (fire_at, Some(now), Some("omitido")) };
            sqlx::query(&format!("INSERT INTO reminder({col},fire_at,rule,fired_at,target_at,title) VALUES(?,?,?,?,?,?)")).bind(id).bind(fire).bind(rule).bind(fired).bind(target_at).bind(title).execute(pool).await?;
        }
        Some((rid, tgt, fired, canc)) => {
            if tgt != Some(target_at) {
                // La fecha cambió: se rearma con la nueva. Si el aviso ya habría pasado, queda omitido (el cambio se notifica aparte).
                if fire_at > now { sqlx::query("UPDATE reminder SET fire_at=?, target_at=?, fired_at=NULL, cancelled_at=NULL, title=NULL WHERE id=?").bind(fire_at).bind(target_at).bind(rid).execute(pool).await?; }
                else { sqlx::query("UPDATE reminder SET fire_at=?, target_at=?, fired_at=?, cancelled_at=NULL, title='omitido' WHERE id=?").bind(fire_at).bind(target_at).bind(now).bind(rid).execute(pool).await?; }
            } else if canc.is_some() && fired.is_none() && fire_at > now {
                sqlx::query("UPDATE reminder SET cancelled_at=NULL WHERE id=?").bind(rid).execute(pool).await?;
            }
        }
    }
    Ok(())
}

async fn cancel_pending(pool: &SqlitePool, owner: Owner, keep_rules: &[String], now: i64) -> AppResult<()> {
    let (col, id) = match owner { Owner::Activity(i) => ("activity_id", i), Owner::Event(i) => ("event_id", i) };
    let rules: Vec<(i64, String)> = sqlx::query_as(&format!("SELECT id, rule FROM reminder WHERE {col}=? AND fired_at IS NULL AND cancelled_at IS NULL")).bind(id).fetch_all(pool).await?;
    for (rid, rule) in rules.into_iter().filter(|(_, r)| !keep_rules.contains(r)) {
        let _ = rule;
        sqlx::query("UPDATE reminder SET cancelled_at=? WHERE id=?").bind(now).bind(rid).execute(pool).await?;
    }
    Ok(())
}

/// Pone al día los avisos pendientes según el estado actual de actividades y eventos.
pub async fn reconcile(pool: &SqlitePool, now: i64, s: &NotifSettings) -> AppResult<()> {
    // --- fechas límite de tareas ---
    let acts: Vec<(i64, Option<i64>, bool, Option<String>, Option<String>, Option<i64>)> = sqlx::query_as(
        "SELECT a.id, a.due_at, (a.removed_at IS NOT NULL OR c.removed_at IS NOT NULL), x.submission_status, x.grading_status, x.activity_id
         FROM activity a JOIN course c ON c.id=a.course_id LEFT JOIN assignment x ON x.activity_id=a.id WHERE a.modname='assign'").fetch_all(pool).await?;
    for (id, due, removed, sub, gr, synced) in acts {
        let state = derive_state(sub.as_deref(), gr.as_deref(), due, now, false, synced.is_some());
        let active = !removed && due.is_some_and(|d| d > now) && !matches!(state, "entregada" | "evaluada" | "sin_sincronizar");
        if !active { cancel_pending(pool, Owner::Activity(id), &[], now).await?; continue; }
        let due = due.unwrap();
        let rules: Vec<(i64, String, i64)> = s.lead_days.iter().map(|d| (*d, format!("due-{d}d"), due - d * 86400)).collect();
        let closest_passed = rules.iter().filter(|(_, _, f)| *f <= now).map(|(d, _, _)| *d).min();
        let fired_before: i64 = sqlx::query_scalar("SELECT count(*) FROM reminder WHERE activity_id=? AND fired_at IS NOT NULL AND coalesce(title,'')!='omitido'").bind(id).fetch_one(pool).await?;
        for (lead, rule, fire_at) in &rules {
            ensure(pool, Owner::Activity(id), rule, *fire_at, due, now, Some(*lead) == closest_passed && fired_before == 0).await?;
        }
        let keep: Vec<String> = rules.into_iter().map(|(_, r, _)| r).collect();
        cancel_pending(pool, Owner::Activity(id), &keep, now).await?;
    }
    // --- exámenes de Moodle (mismos avisos) y eventos personales con aviso previo ---
    let evs: Vec<(i64, String, String, i64, bool, Option<i64>)> = sqlx::query_as("SELECT id, source, kind, starts_at, cancelled_at IS NOT NULL, remind_min FROM calendar_event WHERE starts_at > ?").bind(now - 86400).fetch_all(pool).await?;
    for (id, source, kind, starts, cancelled, remind) in evs {
        let mut keep = vec![];
        if !cancelled && starts > now {
            if source == "moodle" && kind == "exam" {
                for d in &s.lead_days { let rule = format!("due-{d}d"); ensure(pool, Owner::Event(id), &rule, starts - d * 86400, starts, now, false).await?; keep.push(rule); }
            } else if source == "personal" { if let Some(m) = remind {
                ensure(pool, Owner::Event(id), "evt-min", starts - m * 60, starts, now, false).await?; keep.push("evt-min".into());
            } }
        }
        cancel_pending(pool, Owner::Event(id), &keep, now).await?;
    }
    Ok(())
}

/// Formato de fecha/hora local (Madrid) para los textos de las notificaciones.
pub fn fmt_dt(ts: i64) -> String { local(ts).format("%d/%m/%Y %H:%M").to_string() }

/// Crea una notificación de evento (cambio de fecha, nota nueva…) si su categoría está activada.
pub async fn emit(pool: &SqlitePool, category: &str, priority: i64, title: &str, body: Option<&str>, ref_kind: Option<&str>, ref_id: Option<i64>) {
    if !load_settings(pool).await.enabled(category) { return; }
    if let Err(e) = notif::add(pool, category, priority, title, body, ref_kind, ref_id).await { tracing::warn!("crear notificación: {e}"); }
}

fn when_text(days: i64, what: &str) -> String {
    match days { i64::MIN..=0 => format!("{what} hoy"), 1 => format!("{what} mañana"), n => format!("{what} en {n} días") }
}

/// Convierte en notificaciones los avisos vencidos, si ahora es un momento permitido. Cada aviso se dispara una sola vez.
pub async fn fire_due(pool: &SqlitePool, now: i64, s: &NotifSettings) -> AppResult<Vec<i64>> {
    if !allowed_now(s, now) { return Ok(vec![]); }
    let due: Vec<(i64, Option<i64>, Option<i64>, String, i64)> = sqlx::query_as(
        "SELECT id, activity_id, event_id, rule, coalesce(target_at, fire_at) FROM reminder WHERE fire_at<=? AND fired_at IS NULL AND cancelled_at IS NULL ORDER BY fire_at LIMIT 50").bind(now).fetch_all(pool).await?;
    let mut made = vec![];
    for (rid, act, evt, rule, target) in due {
        let claimed = sqlx::query("UPDATE reminder SET fired_at=? WHERE id=? AND fired_at IS NULL AND cancelled_at IS NULL").bind(now).bind(rid).execute(pool).await?.rows_affected();
        if claimed != 1 { continue; }   // otro proceso (app o servicio) ya lo ha disparado
        let days = day_index(target) - day_index(now);
        let prio = if rule == "due-1d" { 2 } else if rule == "due-3d" || rule == "evt-min" { 1 } else { 0 };
        let tl = local(target).format("%d/%m %H:%M").to_string();
        let (category, title, body, rk, ri) = if let Some(a) = act {
            let r: Option<(String, String, Option<String>, Option<String>)> = sqlx::query_as("SELECT a.name, c.fullname, x.submission_status, x.grading_status FROM activity a JOIN course c ON c.id=a.course_id LEFT JOIN assignment x ON x.activity_id=a.id WHERE a.id=?").bind(a).fetch_optional(pool).await?;
            let Some((name, course, sub, gr)) = r else { continue };
            if matches!(derive_state(sub.as_deref(), gr.as_deref(), Some(target), now, false, true), "entregada" | "evaluada") { continue; } // se entregó entre medias
            ("deadline", format!("{}: {}", when_text(days, "Entrega"), name), format!("{course} · cierra el {tl}"), "activity", a)
        } else if let Some(e) = evt {
            let r: Option<(String, String, String)> = sqlx::query_as("SELECT title, kind, source FROM calendar_event WHERE id=?").bind(e).fetch_optional(pool).await?;
            let Some((title, kind, source)) = r else { continue };
            if source == "moodle" && kind == "exam" { ("exam", format!("{}: {}", when_text(days, "Examen"), title.trim_start_matches("Examen: ")), format!("El {tl}"), "event", e) }
            else { ("reminder", title, format!("A las {}", local(target).format("%H:%M")), "event", e) }
        } else { continue };
        if !s.enabled(category) { continue; }
        made.push(notif::add(pool, category, prio, &title, Some(&body), Some(rk), Some(ri)).await?);
    }
    Ok(made)
}

/// Resúmenes diario y semanal (opcionales): una vez por día/semana, a la hora configurada.
pub async fn run_digests(pool: &SqlitePool, now: i64, s: &NotifSettings) -> AppResult<Vec<i64>> {
    let mut made = vec![];
    if !allowed_now(s, now) || !s.enabled("digest") { return Ok(made); }
    let l = local(now);
    let mins = (l.hour() * 60 + l.minute()) as i64;
    let today = l.format("%Y-%m-%d").to_string();
    let pending = |from: i64, to: i64| async move {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT a.name, a.due_at FROM activity a JOIN course c ON c.id=a.course_id LEFT JOIN assignment x ON x.activity_id=a.id
             WHERE a.modname='assign' AND a.removed_at IS NULL AND c.removed_at IS NULL AND x.activity_id IS NOT NULL AND a.due_at >= ?1 AND a.due_at < ?2
               AND lower(coalesce(x.submission_status,'')) NOT LIKE '%enviado%' AND lower(coalesce(x.grading_status,'')) NOT LIKE 'calificado%' ORDER BY a.due_at").bind(from).bind(to).fetch_all(pool).await?;
        Ok::<_, crate::error::AppError>(rows)
    };
    let day0 = TZ.from_local_datetime(&l.date_naive().and_hms_opt(0, 0, 0).unwrap()).earliest().map(|d| d.timestamp()).unwrap_or(now);
    if let Some(t) = s.daily_digest.as_deref().and_then(minutes) {
        if mins >= t && db::get_setting(pool, "notif.last_daily").await?.as_deref() != Some(&today) {
            db::set_setting(pool, "notif.last_daily", &today).await?;
            let rows = pending(day0, day0 + 2 * 86400).await?;
            if !rows.is_empty() {
                let body = rows.iter().map(|(n, d)| format!("• {n} ({})", local(*d).format("%d/%m %H:%M"))).collect::<Vec<_>>().join("\n");
                made.push(notif::add(pool, "digest", 0, &format!("Resumen de hoy: {} entrega(s) hoy o mañana", rows.len()), Some(&body), None, None).await?);
            }
        }
    }
    if let Some(t) = s.weekly_digest.as_deref().and_then(minutes) {
        let week = format!("{}-W{:02}", l.iso_week().year(), l.iso_week().week());
        if l.weekday().num_days_from_monday() as u8 == s.weekly_day && mins >= t && db::get_setting(pool, "notif.last_weekly").await?.as_deref() != Some(&week) {
            db::set_setting(pool, "notif.last_weekly", &week).await?;
            let rows = pending(day0, day0 + 7 * 86400).await?;
            if !rows.is_empty() {
                let body = rows.iter().take(6).map(|(n, d)| format!("• {n} ({})", local(*d).format("%a %d/%m"))).collect::<Vec<_>>().join("\n");
                made.push(notif::add(pool, "digest", 0, &format!("Resumen semanal: {} entrega(s) en los próximos 7 días", rows.len()), Some(&body), None, None).await?);
            }
        }
    }
    Ok(made)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{calendar, moodle::models::*, sync};

    // DATOS DE PRUEBA sintéticos: no proceden de Moodle real.
    const DAY: i64 = 86400;
    // 2026-11-10 10:00 (Madrid, UTC+1) = 09:00 UTC
    fn t0() -> i64 { chrono::Utc.with_ymd_and_hms(2026, 11, 10, 9, 0, 0).unwrap().timestamp() }

    async fn setup(due: Option<i64>, sub: &str) -> (tempfile::TempDir, SqlitePool, i64) {
        let d = tempfile::tempdir().unwrap();
        let p = db::open(&d.path().join("t.db")).await.unwrap();
        sync::apply_courses(&p, &[RemoteCourse { moodle_id: 1, fullname: "Interfaces".into(), shortname: None, url: String::new() }]).await.unwrap();
        let a = RemoteActivity { cmid: 10, modname: "assign".into(), name: "Tarea 1".into(), url: None, due_at: None, files: vec![] };
        sync::apply_contents(&p, 1, &[RemoteSection { moodle_id: Some(9), position: 1, name: "T1".into(), activities: vec![a] }]).await.unwrap();
        let id: i64 = sqlx::query_scalar("SELECT id FROM activity WHERE moodle_cmid=10").fetch_one(&p).await.unwrap();
        sqlx::query("UPDATE activity SET due_at=? WHERE id=?").bind(due).bind(id).execute(&p).await.unwrap();
        sqlx::query("INSERT INTO assignment(activity_id, submission_status, grading_status) VALUES(?,?, 'Sin calificar')").bind(id).bind(sub).execute(&p).await.unwrap();
        (d, p, id)
    }
    const NO_SUB: &str = "Todavía no se han realizado envíos";
    async fn pending(p: &SqlitePool) -> Vec<(String, i64)> { sqlx::query_as("SELECT rule, fire_at FROM reminder WHERE fired_at IS NULL AND cancelled_at IS NULL ORDER BY fire_at").fetch_all(p).await.unwrap() }

    #[tokio::test]
    async fn seven_three_one_days_then_cancelled_when_submitted() {
        let (_d, p, id) = setup(Some(t0() + 20 * DAY), NO_SUB).await;
        let s = NotifSettings::default();
        reconcile(&p, t0(), &s).await.unwrap();
        assert_eq!(pending(&p).await.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(), ["due-7d", "due-3d", "due-1d"]);
        reconcile(&p, t0(), &s).await.unwrap();
        assert_eq!(pending(&p).await.len(), 3, "idempotente");
        sqlx::query("UPDATE assignment SET submission_status='Enviado para calificar' WHERE activity_id=?").bind(id).execute(&p).await.unwrap();
        reconcile(&p, t0(), &s).await.unwrap();
        assert!(pending(&p).await.is_empty(), "entregada: se cancelan los avisos");
    }

    #[tokio::test]
    async fn date_change_rearms_reminders() {
        let (_d, p, id) = setup(Some(t0() + 20 * DAY), NO_SUB).await;
        let s = NotifSettings::default();
        reconcile(&p, t0(), &s).await.unwrap();
        sqlx::query("UPDATE activity SET due_at=? WHERE id=?").bind(t0() + 30 * DAY).bind(id).execute(&p).await.unwrap();
        reconcile(&p, t0(), &s).await.unwrap();
        let r = pending(&p).await;
        assert_eq!(r.len(), 3);
        assert_eq!(r[0], ("due-7d".to_string(), t0() + 23 * DAY), "recalculados con la fecha nueva");
    }

    #[tokio::test]
    async fn first_sight_gives_one_catchup_not_a_burst() {
        let (_d, p, _) = setup(Some(t0() + 2 * DAY), NO_SUB).await;   // quedan 2 días: ya pasaron los avisos de 7 y 3 días
        let s = NotifSettings::default();
        reconcile(&p, t0(), &s).await.unwrap();
        let made = fire_due(&p, t0(), &s).await.unwrap();
        assert_eq!(made.len(), 1, "un solo aviso inicial (el más cercano), no tres");
        let n = notif::list(&p, 10, false).await.unwrap();
        assert!(n[0].title.starts_with("Entrega en 2 días: Tarea 1"), "{}", n[0].title);
        assert_eq!(fire_due(&p, t0() + 60, &s).await.unwrap().len(), 0, "no se repite");
        assert_eq!(pending(&p).await.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(), ["due-1d"]);
    }

    #[tokio::test]
    async fn no_due_date_means_no_reminders() {
        let (_d, p, _) = setup(None, NO_SUB).await;
        reconcile(&p, t0(), &NotifSettings::default()).await.unwrap();
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM reminder").fetch_one(&p).await.unwrap();
        assert_eq!(n, 0, "no se inventa ninguna fecha");
    }

    #[tokio::test]
    async fn quiet_hours_and_weekdays_postpone_instead_of_losing() {
        let (_d, p, _) = setup(Some(t0() + 6 * DAY), NO_SUB).await;
        let s = NotifSettings::default();
        reconcile(&p, t0(), &s).await.unwrap();
        // 7d ya pasado -> catch-up. Se intenta a las 23:30 (franja silenciosa): no sale
        let night = chrono::Utc.with_ymd_and_hms(2026, 11, 10, 22, 30, 0).unwrap().timestamp();   // 23:30 Madrid
        assert!(!allowed_now(&s, night));
        assert!(fire_due(&p, night, &s).await.unwrap().is_empty());
        assert_eq!(pending(&p).await.len(), 3, "sigue pendiente");
        let morning = chrono::Utc.with_ymd_and_hms(2026, 11, 11, 8, 30, 0).unwrap().timestamp(); // 09:30 Madrid
        assert_eq!(fire_due(&p, morning, &s).await.unwrap().len(), 1);
        // día de la semana deshabilitado (miércoles 11 nov): tampoco avisa
        let mut s2 = NotifSettings::default(); s2.weekdays[2] = false;
        assert!(!allowed_now(&s2, morning));
        // categoría desactivada: se consume el aviso pero no se crea notificación
        let mut s3 = NotifSettings::default(); s3.categories.insert("deadline".into(), false);
        let (_d2, p2, _) = setup(Some(t0() + 6 * DAY), NO_SUB).await;
        reconcile(&p2, t0(), &s3).await.unwrap();
        assert!(fire_due(&p2, t0(), &s3).await.unwrap().is_empty());
        assert!(notif::list(&p2, 5, false).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn personal_event_reminder_and_cancel_on_delete() {
        let (_d, p, _) = setup(None, NO_SUB).await;
        let id = calendar::create_personal(&p, &calendar::NewEvent { title: "Sesión de estudio".into(), kind: "study".into(), starts_at: t0() + 3600, ends_at: None, all_day: false, notes: None, remind_min: Some(30) }).await.unwrap();
        let s = NotifSettings::default();
        reconcile(&p, t0(), &s).await.unwrap();
        assert_eq!(pending(&p).await, [("evt-min".to_string(), t0() + 1800)]);
        assert_eq!(fire_due(&p, t0() + 1800, &s).await.unwrap().len(), 1);
        let n = notif::list(&p, 5, false).await.unwrap();
        assert_eq!((n[0].category.as_str(), n[0].title.as_str()), ("reminder", "Sesión de estudio"));
        calendar::delete_personal(&p, id).await.unwrap();
    }

    #[tokio::test]
    async fn digests_are_sent_once_per_day() {
        let (_d, p, _) = setup(Some(t0() + DAY / 2), NO_SUB).await;   // vence hoy a las 22:00
        let s = NotifSettings { daily_digest: Some("08:30".into()), ..Default::default() };
        let early = chrono::Utc.with_ymd_and_hms(2026, 11, 10, 7, 0, 0).unwrap().timestamp();   // 08:00 Madrid: aún no
        assert!(run_digests(&p, early, &s).await.unwrap().is_empty());
        let t = chrono::Utc.with_ymd_and_hms(2026, 11, 10, 8, 40, 0).unwrap().timestamp();      // 09:40 Madrid
        assert_eq!(run_digests(&p, t, &s).await.unwrap().len(), 1);
        assert!(run_digests(&p, t + 600, &s).await.unwrap().is_empty(), "solo una vez al día");
    }

    #[test]
    fn settings_are_validated() {
        let s = NotifSettings { lead_days: vec![1, 99, 3, 3, -2], weekly_day: 9, ..Default::default() }.validated();
        assert_eq!((s.lead_days, s.weekly_day), (vec![3, 1], 6));
        assert!(minutes("25:00").is_none() && minutes("8:5").is_some() && minutes("abc").is_none());
    }
}
