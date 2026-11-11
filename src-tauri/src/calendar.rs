//! Calendario: eventos de Moodle (derivados de las fechas de las actividades + vista «Próximos eventos») y eventos personales.
//! Los eventos de Moodle nunca se modifican en Moodle; las notas locales sobreviven a los cambios de fecha.
use crate::error::{AppError, AppResult};
use crate::moodle::mapper::PageEvent;
use crate::tasks::derive_state;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

pub const PERSONAL_KINDS: [&str; 6] = ["reminder", "study", "personal", "prep", "mock_exam", "goal"];

fn now() -> i64 { chrono::Utc::now().timestamp() }

/// Heurística (documentada): un cuestionario o evento cuyo nombre sugiere examen se marca como «exam».
pub fn looks_like_exam(title: &str) -> bool {
    let t = title.to_lowercase();
    ["examen", "exam", "final test", "prueba escrita", "control "].iter().any(|k| t.contains(k))
}

/// Id negativo y estable para eventos derivados de la fecha de una actividad (1=entrega/cierre, 2=apertura).
fn derived_id(activity_id: i64, slot: i64) -> i64 { -(activity_id * 10 + slot) }

struct Desired { id: i64, kind: &'static str, title: String, starts: i64, ends: Option<i64> }

/// Reconstruye los eventos de Moodle a partir de las actividades sincronizadas. Devuelve cuántos eventos cambiaron.
pub async fn sync_derived(pool: &SqlitePool) -> AppResult<i64> {
    let acts: Vec<(i64, String, String, Option<i64>, Option<i64>, bool)> = sqlx::query_as(
        "SELECT a.id, a.name, a.modname, a.opens_at, a.due_at, (a.removed_at IS NOT NULL OR c.removed_at IS NOT NULL)
         FROM activity a JOIN course c ON c.id=a.course_id WHERE a.modname IN ('assign','quiz')").fetch_all(pool).await?;
    let mut tx = pool.begin().await?;
    let t = now();
    let mut changed = 0;
    for (aid, name, modname, opens, due, removed) in acts {
        let mut want: Vec<Desired> = vec![];
        if !removed {
            if modname == "assign" {
                if let Some(d) = due { want.push(Desired { id: derived_id(aid, 1), kind: "due", title: format!("Entrega: {name}"), starts: d, ends: None }); }
            } else if looks_like_exam(&name) {
                if let Some(o) = opens { want.push(Desired { id: derived_id(aid, 2), kind: "exam", title: format!("Examen: {name}"), starts: o, ends: due }); }
            } else {
                if let Some(o) = opens { want.push(Desired { id: derived_id(aid, 2), kind: "quiz", title: format!("Cuestionario abre: {name}"), starts: o, ends: None }); }
                if let Some(d) = due { want.push(Desired { id: derived_id(aid, 1), kind: "quiz", title: format!("Cuestionario cierra: {name}"), starts: d, ends: None }); }
            }
        }
        let keep: Vec<i64> = want.iter().map(|w| w.id).collect();
        for w in want {
            let old: Option<(i64, Option<i64>)> = sqlx::query_as("SELECT starts_at, cancelled_at FROM calendar_event WHERE source='moodle' AND moodle_event_id=?").bind(w.id).fetch_optional(&mut *tx).await?;
            if old.map_or(true, |(s, c)| s != w.starts || c.is_some()) { changed += 1; }
            sqlx::query("INSERT INTO calendar_event(source,moodle_event_id,activity_id,kind,title,starts_at,ends_at,all_day,last_synced_at) VALUES('moodle',?,?,?,?,?,?,0,?)
                         ON CONFLICT(source,moodle_event_id) DO UPDATE SET kind=excluded.kind, title=excluded.title, starts_at=excluded.starts_at, ends_at=excluded.ends_at, cancelled_at=NULL, last_synced_at=excluded.last_synced_at")
                .bind(w.id).bind(aid).bind(w.kind).bind(&w.title).bind(w.starts).bind(w.ends).bind(t).execute(&mut *tx).await?;
        }
        // Lo que ya no existe en Moodle se conserva marcado como cancelado.
        let existing: Vec<i64> = sqlx::query_scalar("SELECT moodle_event_id FROM calendar_event WHERE source='moodle' AND moodle_event_id<0 AND activity_id=? AND cancelled_at IS NULL").bind(aid).fetch_all(&mut *tx).await?;
        for id in existing.into_iter().filter(|i| !keep.contains(i)) {
            sqlx::query("UPDATE calendar_event SET cancelled_at=? WHERE source='moodle' AND moodle_event_id=?").bind(t).bind(id).execute(&mut *tx).await?;
            changed += 1;
        }
    }
    tx.commit().await?;
    Ok(changed)
}

/// Eventos de la vista «Próximos eventos» de Moodle que NO son ya una fecha de tarea/cuestionario conocida (exámenes, eventos de curso…).
pub async fn apply_page_events(pool: &SqlitePool, events: &[PageEvent]) -> AppResult<i64> {
    let mut tx = pool.begin().await?;
    let t = now();
    let mut changed = 0;
    let mut kept = vec![];
    for e in events {
        if let Some(cmid) = e.cmid {
            let known: i64 = sqlx::query_scalar("SELECT count(*) FROM activity WHERE moodle_cmid=? AND modname IN ('assign','quiz')").bind(cmid).fetch_one(&mut *tx).await?;
            if known > 0 { continue; }   // ya lo cubre el evento derivado de la actividad
        }
        kept.push(e.moodle_id);
        let kind = if looks_like_exam(&e.title) { "exam" } else if e.eventtype == "due" { "due" } else { "course" };
        let old: Option<(i64, Option<i64>)> = sqlx::query_as("SELECT starts_at, cancelled_at FROM calendar_event WHERE source='moodle' AND moodle_event_id=?").bind(e.moodle_id).fetch_optional(&mut *tx).await?;
        if old.map_or(true, |(s, c)| s != e.starts_at || c.is_some()) { changed += 1; }
        sqlx::query("INSERT INTO calendar_event(source,moodle_event_id,kind,title,starts_at,all_day,last_synced_at) VALUES('moodle',?,?,?,?,0,?)
                     ON CONFLICT(source,moodle_event_id) DO UPDATE SET kind=excluded.kind, title=excluded.title, starts_at=excluded.starts_at, cancelled_at=NULL, last_synced_at=excluded.last_synced_at")
            .bind(e.moodle_id).bind(kind).bind(&e.title).bind(e.starts_at).bind(t).execute(&mut *tx).await?;
    }
    // Cancelación solo dentro de la ventana que la vista cubre realmente (hasta su último evento mostrado).
    if let Some(last) = events.iter().map(|e| e.starts_at).max() {
        let inside: Vec<i64> = sqlx::query_scalar("SELECT moodle_event_id FROM calendar_event WHERE source='moodle' AND moodle_event_id>0 AND cancelled_at IS NULL AND starts_at BETWEEN ? AND ?").bind(t).bind(last).fetch_all(&mut *tx).await?;
        for id in inside.into_iter().filter(|i| !kept.contains(i)) {
            sqlx::query("UPDATE calendar_event SET cancelled_at=? WHERE source='moodle' AND moodle_event_id=?").bind(t).bind(id).execute(&mut *tx).await?;
            changed += 1;
        }
    }
    tx.commit().await?;
    Ok(changed)
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct EventBase {
    pub id: i64, pub source: String, pub activity_id: Option<i64>, pub kind: String, pub title: String, pub notes: Option<String>,
    pub starts_at: i64, pub ends_at: Option<i64>, pub all_day: i64, pub cancelled_at: Option<i64>, pub remind_min: Option<i64>,
    pub course_name: Option<String>, pub submission_status: Option<String>, pub grading_status: Option<String>, pub due_at: Option<i64>, pub synced: Option<i64>,
    pub account_id: Option<i64>, pub student_id: Option<i64>, pub student_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EventRow { #[serde(flatten)] pub base: EventBase, pub activity_state: Option<&'static str> }

pub async fn list(pool: &SqlitePool, from: i64, to: i64) -> AppResult<Vec<EventRow>> {
    let rows: Vec<EventBase> = sqlx::query_as(
        "SELECT e.id, e.source, e.activity_id, e.kind, e.title, e.notes, e.starts_at, e.ends_at, e.all_day, e.cancelled_at, e.remind_min,
                c.fullname AS course_name, x.submission_status, x.grading_status, a.due_at, x.activity_id AS synced,
                e.account_id, e.student_id, s.display_name AS student_name
         FROM calendar_event e LEFT JOIN activity a ON a.id=e.activity_id LEFT JOIN course c ON c.id=a.course_id LEFT JOIN assignment x ON x.activity_id=a.id LEFT JOIN student s ON s.id=e.student_id
         WHERE e.starts_at < ?2 AND coalesce(e.ends_at, e.starts_at) >= ?1 ORDER BY e.starts_at, e.id").bind(from).bind(to).fetch_all(pool).await?;
    let t = now();
    Ok(rows.into_iter().map(|b| {
        let st = (b.kind == "due" && b.activity_id.is_some()).then(|| derive_state(b.submission_status.as_deref(), b.grading_status.as_deref(), b.due_at, t, false, b.synced.is_some()));
        EventRow { base: b, activity_state: st }
    }).collect())
}

#[derive(Debug, Deserialize)]
pub struct NewEvent { pub title: String, pub kind: String, pub starts_at: i64, pub ends_at: Option<i64>, #[serde(default)] pub all_day: bool, pub notes: Option<String>, pub remind_min: Option<i64> }

fn check(e: &NewEvent) -> AppResult<()> {
    if e.title.trim().is_empty() { return Err(AppError::Other("El evento necesita un título.".into())); }
    if !PERSONAL_KINDS.contains(&e.kind.as_str()) { return Err(AppError::Other(format!("Tipo de evento no válido: {}", e.kind))); }
    if let Some(end) = e.ends_at { if end < e.starts_at { return Err(AppError::Other("El evento no puede terminar antes de empezar.".into())); } }
    if e.remind_min.is_some_and(|m| !(0..=60 * 24 * 60).contains(&m)) { return Err(AppError::Other("El aviso previo debe estar entre 0 y 60 días.".into())); }
    Ok(())
}

pub async fn create_personal(pool: &SqlitePool, e: &NewEvent) -> AppResult<i64> {
    check(e)?;
    Ok(sqlx::query_scalar("INSERT INTO calendar_event(source,kind,title,notes,starts_at,ends_at,all_day,remind_min) VALUES('personal',?,?,?,?,?,?,?) RETURNING id")
        .bind(&e.kind).bind(e.title.trim()).bind(&e.notes).bind(e.starts_at).bind(e.ends_at).bind(e.all_day as i64).bind(e.remind_min).fetch_one(pool).await?)
}

pub async fn update_personal(pool: &SqlitePool, id: i64, e: &NewEvent) -> AppResult<()> {
    check(e)?;
    let n = sqlx::query("UPDATE calendar_event SET kind=?, title=?, notes=?, starts_at=?, ends_at=?, all_day=?, remind_min=? WHERE id=? AND source='personal'")
        .bind(&e.kind).bind(e.title.trim()).bind(&e.notes).bind(e.starts_at).bind(e.ends_at).bind(e.all_day as i64).bind(e.remind_min).bind(id).execute(pool).await?.rows_affected();
    if n == 0 { return Err(AppError::Other("Solo se pueden editar los eventos personales (los de Moodle no se modifican).".into())); }
    // los avisos del evento se recalculan con la nueva fecha
    sqlx::query("UPDATE reminder SET cancelled_at=strftime('%s','now') WHERE event_id=? AND fired_at IS NULL AND cancelled_at IS NULL").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn delete_personal(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let n = sqlx::query("DELETE FROM calendar_event WHERE id=? AND source='personal'").bind(id).execute(pool).await?.rows_affected();
    if n == 0 { return Err(AppError::Other("Solo se pueden borrar los eventos personales.".into())); }
    Ok(())
}

/// Las notas locales se pueden añadir a cualquier evento, también a los de Moodle.
pub async fn set_notes(pool: &SqlitePool, id: i64, notes: &str) -> AppResult<()> {
    sqlx::query("UPDATE calendar_event SET notes=? WHERE id=?").bind(if notes.trim().is_empty() { None } else { Some(notes) }).bind(id).execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, moodle::models::*, sync};

    // DATOS DE PRUEBA sintéticos: no proceden de Moodle real.
    async fn setup() -> (tempfile::TempDir, SqlitePool) {
        let d = tempfile::tempdir().unwrap();
        let p = db::open(&d.path().join("t.db")).await.unwrap();
        sync::apply_courses(&p, &[RemoteCourse { moodle_id: 1, fullname: "Desarrollo de Interfaces".into(), shortname: None, url: String::new() }]).await.unwrap();
        let act = |c, m: &str, n: &str| RemoteActivity { cmid: c, modname: m.into(), name: n.into(), url: None, due_at: None, files: vec![] };
        sync::apply_contents(&p, 1, &[RemoteSection { moodle_id: Some(9), position: 1, name: "T1".into(), activities: vec![act(10, "assign", "Tarea 1"), act(11, "quiz", "Examen UD1"), act(12, "quiz", "Cuestionario previo")] }]).await.unwrap();
        (d, p)
    }
    async fn set_dates(p: &SqlitePool, cmid: i64, opens: Option<i64>, due: Option<i64>) { sqlx::query("UPDATE activity SET opens_at=?, due_at=? WHERE moodle_cmid=?").bind(opens).bind(due).bind(cmid).execute(p).await.unwrap(); }

    #[tokio::test]
    async fn derived_events_follow_activity_dates_and_keep_local_notes() {
        let (_d, p) = setup().await;
        set_dates(&p, 10, None, Some(5_000_000)).await;
        set_dates(&p, 11, Some(6_000_000), Some(6_003_600)).await;
        set_dates(&p, 12, Some(7_000_000), Some(7_100_000)).await;
        assert_eq!(sync_derived(&p).await.unwrap(), 4);
        assert_eq!(sync_derived(&p).await.unwrap(), 0, "idempotente: sin duplicados");
        let ev = list(&p, 0, 10_000_000).await.unwrap();
        let kinds: Vec<(&str, &str)> = ev.iter().map(|e| (e.base.kind.as_str(), e.base.title.as_str())).collect();
        assert_eq!(kinds, [("due", "Entrega: Tarea 1"), ("exam", "Examen: Examen UD1"), ("quiz", "Cuestionario abre: Cuestionario previo"), ("quiz", "Cuestionario cierra: Cuestionario previo")]);
        let id = ev[0].base.id;
        set_notes(&p, id, "Hacerlo con calma").await.unwrap();
        set_dates(&p, 10, None, Some(5_500_000)).await;            // el profesor cambia la fecha
        assert_eq!(sync_derived(&p).await.unwrap(), 1);
        let ev = list(&p, 0, 10_000_000).await.unwrap();
        assert_eq!((ev[0].base.starts_at, ev[0].base.notes.as_deref()), (5_500_000, Some("Hacerlo con calma")), "fecha nueva, nota conservada");
        set_dates(&p, 10, None, None).await;                         // la fecha desaparece: se conserva marcada como cancelada
        sync_derived(&p).await.unwrap();
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM calendar_event WHERE cancelled_at IS NOT NULL").fetch_one(&p).await.unwrap();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn page_events_do_not_duplicate_known_activities() {
        let (_d, p) = setup().await;
        let ev = |id, title: &str, cmid, ts| PageEvent { moodle_id: id, title: title.into(), component: "".into(), eventtype: "course".into(), course_id: Some(1), starts_at: ts, cmid };
        let evs = vec![ev(1, "Vencimiento de Tarea 1", Some(10), 9_000_000), ev(2, "Examen de febrero", None, 9_500_000), ev(3, "Charla del centro", None, 9_600_000)];
        assert_eq!(apply_page_events(&p, &evs).await.unwrap(), 2);
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM calendar_event WHERE moodle_event_id>0").fetch_one(&p).await.unwrap();
        assert_eq!(n, 2, "el de la tarea conocida (cmid 10) se omite");
        let k: String = sqlx::query_scalar("SELECT kind FROM calendar_event WHERE moodle_event_id=2").fetch_one(&p).await.unwrap();
        assert_eq!(k, "exam");
        assert_eq!(apply_page_events(&p, &evs).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn personal_events_are_freely_editable_moodle_ones_are_not() {
        let (_d, p) = setup().await;
        set_dates(&p, 10, None, Some(5_000_000)).await;
        sync_derived(&p).await.unwrap();
        let moodle_id: i64 = sqlx::query_scalar("SELECT id FROM calendar_event WHERE source='moodle'").fetch_one(&p).await.unwrap();
        let ne = NewEvent { title: "Repasar UD1".into(), kind: "study".into(), starts_at: 1_000, ends_at: Some(2_000), all_day: false, notes: None, remind_min: Some(30) };
        let id = create_personal(&p, &ne).await.unwrap();
        update_personal(&p, id, &NewEvent { title: "Repasar UD1 y UD2".into(), ..ne }).await.unwrap();
        assert!(update_personal(&p, moodle_id, &NewEvent { title: "x".into(), kind: "study".into(), starts_at: 1, ends_at: None, all_day: false, notes: None, remind_min: None }).await.is_err());
        assert!(delete_personal(&p, moodle_id).await.is_err());
        delete_personal(&p, id).await.unwrap();
        assert!(create_personal(&p, &NewEvent { title: " ".into(), kind: "study".into(), starts_at: 1, ends_at: None, all_day: false, notes: None, remind_min: None }).await.is_err());
        assert!(create_personal(&p, &NewEvent { title: "x".into(), kind: "inventado".into(), starts_at: 1, ends_at: None, all_day: false, notes: None, remind_min: None }).await.is_err());
        assert!(create_personal(&p, &NewEvent { title: "x".into(), kind: "goal".into(), starts_at: 10, ends_at: Some(5), all_day: false, notes: None, remind_min: None }).await.is_err());
    }
}
