//! Dashboard: situación académica de un vistazo. Todo se calcula desde la BD local (funciona sin conexión).
use crate::calendar::{self, EventRow};
use crate::error::AppResult;
use crate::state::AppState;
use crate::tasks::{list_tasks, TaskRow};
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::State;

const DAY: i64 = 86400;

/// Vencidas hace más de 30 días (restos de cursos pasados): no cuentan como pendientes ni como urgentes.
pub fn is_stale(t: &TaskRow) -> bool { t.state == "fuera_de_plazo" && t.seconds_to_due.is_some_and(|s| s <= -30 * DAY) }

#[derive(Serialize, Debug, PartialEq)]
pub struct PriorityItem { pub id: i64, pub name: String, pub course: String, pub due_at: Option<i64>, pub state: &'static str, pub priority: i64, pub score: i64, pub reason: String }

/// Puntuación transparente: cercanía de la fecha límite (100 vencida · 90 <2 d · 70 <7 d · 50 <14 d · 30 <30 d) + 10 si hay entrega preparada
/// (+8 borrador) + 6 por nivel de prioridad personal (máx. 18: matiza dentro de un tramo, no lo salta). Lo reciente NO suma.
pub fn score(t: &TaskRow) -> (i64, String) {
    let (base, why) = match (t.state, t.seconds_to_due) {
        ("fuera_de_plazo", Some(s)) => (100, format!("vencida hace {} d", (-s) / DAY)),
        (_, Some(s)) if s < 2 * DAY => (90, if s < 3600 { "vence en menos de 1 h".to_string() } else if s < DAY { format!("vence en {} h", s / 3600) } else { "vence mañana".into() }),
        (_, Some(s)) if s < 7 * DAY => (70, format!("vence en {} d", s / DAY)),
        (_, Some(s)) if s < 14 * DAY => (50, format!("vence en {} d", s / DAY)),
        (_, Some(s)) if s < 30 * DAY => (30, format!("vence en {} d", s / DAY)),
        (_, Some(s)) => (10, format!("vence en {} d", s / DAY)),
        (_, None) => (5, "sin fecha límite".into()),
    };
    let state_bonus = match t.state { "preparada" => 10, "iniciada" => 8, _ => 0 };
    let extra = if t.state == "preparada" { " · entrega preparada, falta enviarla" } else if t.state == "iniciada" { " · borrador sin enviar" } else { "" };
    (base + state_bonus + 6 * t.raw.priority, format!("{why}{extra}"))
}

#[derive(Serialize)] pub struct Counts { pub courses: i64, pub todo: i64, pub overdue_recent: i64, pub submitted: i64, pub graded: i64, pub undated: i64, pub stale: i64, pub exams_30d: i64 }
#[derive(Serialize)] pub struct CourseBar { pub course_id: i64, pub name: String, pub done: i64, pub todo: i64 }
#[derive(Serialize)] pub struct Recent { pub kind: String, pub title: String, pub detail: Option<String>, pub at: i64 }
#[derive(Serialize)]
pub struct Dashboard { pub counts: Counts, pub priority: Vec<PriorityItem>, pub events: Vec<EventRow>, pub recent: Vec<Recent>, pub per_course: Vec<CourseBar>, pub last_sync_at: Option<i64>, pub synced_tasks: i64, pub total_tasks: i64 }

pub async fn build(pool: &SqlitePool, now: i64) -> AppResult<Dashboard> {
    let tasks = list_tasks(pool, now).await?;
    let courses: i64 = sqlx::query_scalar("SELECT count(*) FROM course WHERE removed_at IS NULL").fetch_one(pool).await?;
    let mut c = Counts { courses, todo: 0, overdue_recent: 0, submitted: 0, graded: 0, undated: 0, stale: 0, exams_30d: 0 };
    let mut bars: std::collections::BTreeMap<i64, CourseBar> = Default::default();
    let mut prio: Vec<PriorityItem> = vec![];
    for t in &tasks {
        let stale = is_stale(t);
        match t.state {
            "entregada" => c.submitted += 1,
            "evaluada" => { c.graded += 1; }
            "sin_sincronizar" => {}
            _ if stale => c.stale += 1,
            "fuera_de_plazo" => { c.todo += 1; c.overdue_recent += 1; }
            "sin_fecha" => { c.todo += 1; c.undated += 1; }
            _ => c.todo += 1,
        }
        if t.state != "sin_sincronizar" && !stale {
            let b = bars.entry(t.raw.course_id).or_insert(CourseBar { course_id: t.raw.course_id, name: t.raw.course_name.clone(), done: 0, todo: 0 });
            if matches!(t.state, "entregada" | "evaluada") { b.done += 1 } else { b.todo += 1 }
        }
        if !matches!(t.state, "entregada" | "evaluada" | "sin_sincronizar") && !stale {
            let (sc, reason) = score(t);
            prio.push(PriorityItem { id: t.raw.id, name: t.raw.name.clone(), course: t.raw.course_name.clone(), due_at: t.raw.due_at, state: t.state, priority: t.raw.priority, score: sc, reason });
        }
    }
    prio.sort_by(|a, b| b.score.cmp(&a.score).then(a.due_at.unwrap_or(i64::MAX).cmp(&b.due_at.unwrap_or(i64::MAX))));
    prio.truncate(6);
    let events: Vec<EventRow> = calendar::list(pool, now, now + 14 * DAY).await?.into_iter().filter(|e| e.base.cancelled_at.is_none() && e.base.starts_at >= now && !matches!(e.activity_state, Some("entregada" | "evaluada"))).take(8).collect();
    c.exams_30d = sqlx::query_scalar("SELECT count(*) FROM calendar_event WHERE kind IN ('exam','mock_exam') AND cancelled_at IS NULL AND starts_at BETWEEN ? AND ?").bind(now).bind(now + 30 * DAY).fetch_one(pool).await?;

    let mut recent: Vec<Recent> = sqlx::query_as::<_, (String, String, Option<String>, i64)>("SELECT category, title, body, created_at FROM notification WHERE category IN ('new_activity','date_changed','grade','submission') ORDER BY created_at DESC LIMIT 8")
        .fetch_all(pool).await?.into_iter().map(|(k, t, d, at)| Recent { kind: k, title: t, detail: d, at }).collect();
    recent.extend(sqlx::query_as::<_, (String, i64)>("SELECT original_name, downloaded_at FROM local_file WHERE sync_state IN ('synced','outdated') ORDER BY downloaded_at DESC LIMIT 4").fetch_all(pool).await?.into_iter().map(|(n, at)| Recent { kind: "download".into(), title: format!("Documento guardado: {n}"), detail: None, at }));
    recent.sort_by(|a, b| b.at.cmp(&a.at));
    recent.truncate(8);

    let last_sync_at: Option<i64> = sqlx::query_scalar("SELECT max(finished_at) FROM sync_job WHERE state IN ('done','failed') AND kind='full'").fetch_one(pool).await?;
    let synced_tasks = tasks.iter().filter(|t| t.state != "sin_sincronizar").count() as i64;
    Ok(Dashboard { counts: c, priority: prio, events, recent, per_course: bars.into_values().collect(), last_sync_at, synced_tasks, total_tasks: tasks.len() as i64 })
}

#[tauri::command]
pub async fn dashboard_data(st: State<'_, AppState>) -> AppResult<Dashboard> { build(&st.pool, chrono::Utc::now().timestamp()).await }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, moodle::models::*, sync};

    // DATOS DE PRUEBA sintéticos: no proceden de Moodle real.
    const NOW: i64 = 1_800_000_000;
    async fn setup(tasks: &[(&str, Option<i64>, &str, &str, i64)]) -> (tempfile::TempDir, SqlitePool) {
        let d = tempfile::tempdir().unwrap();
        let p = db::open(&d.path().join("t.db")).await.unwrap();
        sync::apply_courses(&p, &[RemoteCourse { moodle_id: 1, fullname: "Interfaces".into(), shortname: None, url: String::new() }]).await.unwrap();
        let acts: Vec<RemoteActivity> = tasks.iter().enumerate().map(|(i, t)| RemoteActivity { cmid: 100 + i as i64, modname: "assign".into(), name: t.0.into(), url: None, due_at: None, files: vec![] }).collect();
        sync::apply_contents(&p, 1, &[RemoteSection { moodle_id: Some(9), position: 1, name: "T".into(), activities: acts }]).await.unwrap();
        for (i, t) in tasks.iter().enumerate() {
            let id: i64 = sqlx::query_scalar("SELECT id FROM activity WHERE moodle_cmid=?").bind(100 + i as i64).fetch_one(&p).await.unwrap();
            sqlx::query("UPDATE activity SET due_at=?, priority=? WHERE id=?").bind(t.1).bind(t.4).bind(id).execute(&p).await.unwrap();
            sqlx::query("INSERT INTO assignment(activity_id, submission_status, grading_status, last_synced_at) VALUES(?,?,?,?)").bind(id).bind(t.2).bind(t.3).bind(NOW).execute(&p).await.unwrap();
        }
        (d, p)
    }
    const NO: &str = "Todavía no se han realizado envíos"; const YES: &str = "Enviado para calificar"; const SC: &str = "Sin calificar";

    #[tokio::test]
    async fn counts_priorities_and_exclusions() {
        let (_d, p) = setup(&[
            ("mañana", Some(NOW + DAY / 2 + 3600), NO, SC, 0), ("en 5 días", Some(NOW + 5 * DAY), NO, SC, 0), ("en 5 días importante", Some(NOW + 5 * DAY), NO, SC, 3),
            ("entregada", Some(NOW + DAY), YES, SC, 0), ("evaluada", Some(NOW - DAY), YES, "Calificado", 0),
            ("vencida reciente", Some(NOW - 3 * DAY), NO, SC, 0), ("vencida antigua", Some(NOW - 400 * DAY), NO, SC, 0), ("sin fecha", None, NO, SC, 0),
        ]).await;
        let d = build(&p, NOW).await.unwrap();
        assert_eq!((d.counts.submitted, d.counts.graded, d.counts.stale, d.counts.overdue_recent, d.counts.undated), (1, 1, 1, 1, 1));
        assert_eq!(d.counts.todo, 5, "mañana, 2×5 días, vencida reciente, sin fecha; la antigua no cuenta");
        let names: Vec<&str> = d.priority.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["vencida reciente", "mañana", "en 5 días importante", "en 5 días", "sin fecha"], "{names:?}");
        assert!(!names.contains(&"vencida antigua") && !names.contains(&"entregada"));
        assert_eq!(d.per_course.len(), 1); assert_eq!((d.per_course[0].done, d.per_course[0].todo), (2, 5));
    }

    #[tokio::test]
    async fn recency_never_raises_priority_but_personal_priority_does() {
        let (_d, p) = setup(&[("reciente sin fecha", None, NO, SC, 0), ("sin fecha pero importante", None, NO, SC, 3)]).await;
        sqlx::query("UPDATE activity SET first_seen_at=? WHERE name='reciente sin fecha'").bind(NOW).execute(&p).await.unwrap();
        let d = build(&p, NOW).await.unwrap();
        assert_eq!(d.priority[0].name, "sin fecha pero importante");
        assert!(d.priority.iter().all(|x| x.reason.contains("sin fecha límite")), "no se inventa urgencia");
    }

    #[tokio::test]
    async fn empty_database_is_fine() {
        let d = tempfile::tempdir().unwrap();
        let p = db::open(&d.path().join("t.db")).await.unwrap();
        let x = build(&p, NOW).await.unwrap();
        assert_eq!((x.counts.todo, x.priority.len(), x.events.len(), x.total_tasks), (0, 0, 0, 0));
    }
}
