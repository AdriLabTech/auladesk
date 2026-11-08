//! MoodleSyncManager (parte de persistencia): aplica extracciones validadas a SQLite.
//! Reglas: nunca borra (usa removed_at), conserva datos locales (favoritos, notas, estado local),
//! y rechaza extracciones sospechosas de estar incompletas antes de tocar nada.
use crate::error::{AppError, AppResult};
use crate::files::sha256_bytes;
use crate::moodle::models::*;
use crate::paths::sanitize_component;
use sqlx::SqlitePool;

#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct SyncStats { pub seen: i64, pub changed: i64, pub removed: i64 }

fn now() -> i64 { chrono::Utc::now().timestamp() }

pub async fn log_history(pool: &SqlitePool, level: &str, msg: &str, selector: Option<&str>, stats: Option<&SyncStats>) {
    let _ = sqlx::query("INSERT INTO sync_history(at,level,message,selector_version,items_seen,items_changed) VALUES(?,?,?,?,?,?)")
        .bind(now()).bind(level).bind(msg).bind(selector).bind(stats.map(|s| s.seen)).bind(stats.map(|s| s.changed))
        .execute(pool).await;
}

pub async fn apply_courses(pool: &SqlitePool, courses: &[RemoteCourse]) -> AppResult<SyncStats> {
    let mut tx = pool.begin().await?;
    let t = now();
    let mut st = SyncStats { seen: courses.len() as i64, ..Default::default() };
    for c in courses {
        let existing: Option<(String, Option<i64>)> = sqlx::query_as("SELECT fullname, removed_at FROM course WHERE moodle_id=?")
            .bind(c.moodle_id).fetch_optional(&mut *tx).await?;
        match existing {
            Some((name, removed)) => {
                if name != c.fullname || removed.is_some() { st.changed += 1; }
                sqlx::query("UPDATE course SET fullname=?, url=?, removed_at=NULL, last_synced_at=? WHERE moodle_id=?")
                    .bind(&c.fullname).bind(&c.url).bind(t).bind(c.moodle_id).execute(&mut *tx).await?;
            }
            None => {
                st.changed += 1;
                let mut folder = sanitize_component(&c.fullname);
                let clash: i64 = sqlx::query_scalar("SELECT count(*) FROM course WHERE folder_name=?").bind(&folder).fetch_one(&mut *tx).await?;
                if clash > 0 { folder = format!("{folder}_{}", c.moodle_id); }
                sqlx::query("INSERT INTO course(moodle_id,fullname,url,folder_name,last_synced_at) VALUES(?,?,?,?,?)")
                    .bind(c.moodle_id).bind(&c.fullname).bind(&c.url).bind(folder).bind(t).execute(&mut *tx).await?;
            }
        }
    }
    // Ausentes: se marcan, no se borran (puede ser un curso oculto o archivado).
    let ids: Vec<i64> = courses.iter().map(|c| c.moodle_id).collect();
    let current: Vec<i64> = sqlx::query_scalar("SELECT moodle_id FROM course WHERE removed_at IS NULL").fetch_all(&mut *tx).await?;
    for id in current.into_iter().filter(|i| !ids.contains(i)) {
        sqlx::query("UPDATE course SET removed_at=? WHERE moodle_id=?").bind(t).bind(id).execute(&mut *tx).await?;
        st.removed += 1;
    }
    tx.commit().await?;
    Ok(st)
}

pub async fn apply_contents(pool: &SqlitePool, course_moodle_id: i64, sections: &[RemoteSection]) -> AppResult<SyncStats> {
    let course_id: i64 = sqlx::query_scalar("SELECT id FROM course WHERE moodle_id=?").bind(course_moodle_id)
        .fetch_optional(pool).await?.ok_or_else(|| AppError::Other(format!("curso {course_moodle_id} desconocido: sincroniza la lista primero")))?;
    let total_acts: usize = sections.iter().map(|s| s.activities.len()).sum();
    let known: i64 = sqlx::query_scalar("SELECT count(*) FROM activity WHERE course_id=? AND removed_at IS NULL").bind(course_id).fetch_one(pool).await?;
    if total_acts == 0 && known > 0 {
        return Err(AppError::Other(format!("extracción rechazada: 0 actividades pero había {known}; no se sobrescribe")));
    }
    // «Ya conocíamos el contenido» = el curso tenía actividades guardadas (la lista de cursos también marca last_synced_at, no sirve).
    let synced_before: bool = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM activity WHERE course_id=?").bind(course_id).fetch_one(pool).await? > 0;
    let course_name: String = sqlx::query_scalar("SELECT fullname FROM course WHERE id=?").bind(course_id).fetch_one(pool).await?;
    let mut new_items: Vec<(String, String)> = vec![];
    let mut tx = pool.begin().await?;
    let t = now();
    let mut st = SyncStats { seen: total_acts as i64, ..Default::default() };
    let mut live_sections = Vec::new();
    let mut live_cmids = Vec::new();
    for s in sections {
        live_sections.push(s.position);
        sqlx::query("INSERT INTO course_section(course_id,moodle_id,position,name,removed_at,last_synced_at) VALUES(?,?,?,?,NULL,?)
                     ON CONFLICT(course_id,position) DO UPDATE SET moodle_id=excluded.moodle_id, name=excluded.name, removed_at=NULL, last_synced_at=excluded.last_synced_at")
            .bind(course_id).bind(s.moodle_id).bind(s.position).bind(&s.name).bind(t).execute(&mut *tx).await?;
        let section_id: i64 = sqlx::query_scalar("SELECT id FROM course_section WHERE course_id=? AND position=?").bind(course_id).bind(s.position).fetch_one(&mut *tx).await?;
        for a in &s.activities {
            live_cmids.push(a.cmid);
            let hash = sha256_bytes(format!("{}|{}|{}|{}", a.modname, a.name, a.url.as_deref().unwrap_or(""), section_id).as_bytes());
            let old: Option<(Option<String>, Option<i64>)> = sqlx::query_as("SELECT content_hash, removed_at FROM activity WHERE course_id=? AND moodle_cmid=?")
                .bind(course_id).bind(a.cmid).fetch_optional(&mut *tx).await?;
            if old.is_none() { new_items.push((a.modname.clone(), a.name.clone())); }
            if old.as_ref().map_or(true, |(h, r)| h.as_deref() != Some(&hash) || r.is_some()) { st.changed += 1; }
            sqlx::query("INSERT INTO activity(course_id,section_id,moodle_cmid,modname,name,url,content_hash,removed_at,last_synced_at,first_seen_at) VALUES(?,?,?,?,?,?,?,NULL,?,?)
                         ON CONFLICT(course_id,moodle_cmid) DO UPDATE SET section_id=excluded.section_id, modname=excluded.modname, name=excluded.name,
                         url=excluded.url, content_hash=excluded.content_hash, removed_at=NULL, last_synced_at=excluded.last_synced_at")
                .bind(course_id).bind(section_id).bind(a.cmid).bind(&a.modname).bind(&a.name).bind(&a.url).bind(&hash).bind(t).bind(t)
                .execute(&mut *tx).await?;
        }
    }
    let gone_acts: Vec<i64> = sqlx::query_scalar("SELECT moodle_cmid FROM activity WHERE course_id=? AND removed_at IS NULL").bind(course_id).fetch_all(&mut *tx).await?;
    for c in gone_acts.into_iter().filter(|c| !live_cmids.contains(c)) {
        sqlx::query("UPDATE activity SET removed_at=? WHERE course_id=? AND moodle_cmid=?").bind(t).bind(course_id).bind(c).execute(&mut *tx).await?;
        st.removed += 1;
    }
    let gone_secs: Vec<i64> = sqlx::query_scalar("SELECT position FROM course_section WHERE course_id=? AND removed_at IS NULL").bind(course_id).fetch_all(&mut *tx).await?;
    for p in gone_secs.into_iter().filter(|p| !live_sections.contains(p)) {
        sqlx::query("UPDATE course_section SET removed_at=? WHERE course_id=? AND position=?").bind(t).bind(course_id).bind(p).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE course SET last_synced_at=? WHERE id=?").bind(t).bind(course_id).execute(&mut *tx).await?;
    tx.commit().await?;
    // Novedades (no en la primera sincronización del curso, para no avisar de todo lo que ya existía).
    if synced_before {
        for (modname, name) in new_items {
            let label = match modname.as_str() { "assign" => "Nueva tarea", "quiz" => "Nuevo cuestionario", "resource" | "folder" => "Nuevo recurso", _ => continue };
            crate::reminders::emit(pool, "new_activity", 0, &format!("{label}: {name}"), Some(&course_name), None, None).await;
        }
    }
    Ok(st)
}

pub async fn apply_assignment(pool: &SqlitePool, a: &crate::moodle::mapper::AssignmentInfo) -> AppResult<SyncStats> {
    let row: Option<(i64, Option<i64>, Option<i64>, String)> = sqlx::query_as("SELECT id, due_at, opens_at, name FROM activity WHERE moodle_cmid=? AND modname='assign' AND removed_at IS NULL")
        .bind(a.cmid).fetch_optional(pool).await?;
    let (id, old_due, old_open, name) = row.ok_or_else(|| AppError::Other(format!("tarea cmid {} desconocida: sincroniza los cursos primero", a.cmid)))?;
    let old_grade: Option<Option<String>> = sqlx::query_scalar("SELECT grade FROM assignment WHERE activity_id=?").bind(id).fetch_optional(pool).await?;
    let mut tx = pool.begin().await?;
    let t = now();
    let mut st = SyncStats { seen: 1, ..Default::default() };
    if old_due != a.due_at || old_open != a.opens_at { st.changed = 1; }
    sqlx::query("UPDATE activity SET opens_at=?, due_at=?, moodle_status=?, last_synced_at=? WHERE id=?")
        .bind(a.opens_at).bind(a.due_at).bind(&a.submission_status).bind(t).bind(id).execute(&mut *tx).await?;
    let files = serde_json::to_string(&a.files).unwrap_or_else(|_| "[]".into());
    sqlx::query("INSERT INTO assignment(activity_id,submission_status,grading_status,grade,feedback,graded_at,grader,time_remaining_text,last_modified_at,submitted_files_json,can_submit,raw_rows_json,last_synced_at,intro_text,intro_links_json)
                 VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
                 ON CONFLICT(activity_id) DO UPDATE SET submission_status=excluded.submission_status, grading_status=excluded.grading_status, grade=excluded.grade, feedback=excluded.feedback,
                 graded_at=excluded.graded_at, grader=excluded.grader, time_remaining_text=excluded.time_remaining_text, last_modified_at=excluded.last_modified_at,
                 submitted_files_json=excluded.submitted_files_json, can_submit=excluded.can_submit, raw_rows_json=excluded.raw_rows_json, last_synced_at=excluded.last_synced_at, intro_text=coalesce(excluded.intro_text,assignment.intro_text), intro_links_json=excluded.intro_links_json")
        .bind(id).bind(&a.submission_status).bind(&a.grading_status).bind(&a.grade).bind(&a.feedback).bind(a.graded_at).bind(&a.grader).bind(&a.time_remaining)
        .bind(a.last_modified_at).bind(files).bind(a.can_submit as i64).bind(&a.raw_rows).bind(t)
        .bind(&a.intro_text).bind(serde_json::to_string(&a.intro_links).unwrap_or_else(|_| "[]".into())).execute(&mut *tx).await?;
    // La nota/comentario de Moodle se refleja en la última versión confirmada de esa tarea.
    sqlx::query("UPDATE submission_version SET grade=coalesce(?,grade), teacher_feedback=coalesce(?,teacher_feedback)
                 WHERE state='confirmed' AND id=(SELECT v.id FROM submission_version v JOIN submission su ON su.id=v.submission_id WHERE su.activity_id=? AND v.state='confirmed' ORDER BY v.id DESC LIMIT 1)")
        .bind(&a.grade).bind(&a.feedback).bind(id).execute(&mut *tx).await?;
    // Un cambio de fecha límite se registra: lo usarán los recordatorios (Fase 5).
    if old_due.is_some() && old_due != a.due_at {
        sqlx::query("INSERT INTO sync_history(at,level,message,selector_version,items_seen,items_changed) VALUES(?,?,?,?,1,1)")
            .bind(t).bind("change").bind(format!("due_changed|{id}|{}|{}|{name}", old_due.unwrap_or(0), a.due_at.unwrap_or(0))).bind("v1").execute(&mut *tx).await?;
    }
    tx.commit().await?;
    // Avisos de cambios (solo si ya conocíamos la tarea: la primera lectura no avisa de lo que ya existía).
    if let (Some(od), true) = (old_due, old_due != a.due_at) {
        let nd = a.due_at.map(crate::reminders::fmt_dt).unwrap_or_else(|| "sin fecha".into());
        crate::reminders::emit(pool, "date_changed", 1, &format!("Cambio de fecha: {name}"), Some(&format!("Antes {} · ahora {nd}", crate::reminders::fmt_dt(od))), Some("activity"), Some(id)).await;
    }
    if let (Some(old), Some(g)) = (&old_grade, &a.grade) {
        if old.as_deref() != Some(g.as_str()) { crate::reminders::emit(pool, "grade", 1, &format!("Nueva calificación: {name}"), Some(g), Some("activity"), Some(id)).await; }
    }
    Ok(st)
}

/// Fechas de actividades que no son tareas (cuestionarios): apertura y cierre.
pub async fn apply_activity_dates(pool: &SqlitePool, cmid: i64, opens: Option<i64>, close: Option<i64>) -> AppResult<SyncStats> {
    let row: Option<(i64, Option<i64>, Option<i64>)> = sqlx::query_as("SELECT id, opens_at, due_at FROM activity WHERE moodle_cmid=? AND removed_at IS NULL").bind(cmid).fetch_optional(pool).await?;
    let (id, o, c) = row.ok_or_else(|| AppError::Other(format!("actividad cmid {cmid} desconocida: sincroniza los cursos primero")))?;
    sqlx::query("UPDATE activity SET opens_at=?, due_at=?, last_synced_at=? WHERE id=?").bind(opens).bind(close).bind(now()).bind(id).execute(pool).await?;
    Ok(SyncStats { seen: 1, changed: (o != opens || c != close) as i64, removed: 0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    // DATOS DE PRUEBA sintéticos: no proceden de Moodle real.
    fn course(id: i64, n: &str) -> RemoteCourse { RemoteCourse { moodle_id: id, fullname: n.into(), shortname: None, url: format!("https://x/course/view.php?id={id}") } }
    fn act(cmid: i64, n: &str) -> RemoteActivity { RemoteActivity { cmid, modname: "assign".into(), name: n.into(), url: None, due_at: None, files: vec![] } }
    fn sec(pos: i64, acts: Vec<RemoteActivity>) -> RemoteSection { RemoteSection { moodle_id: Some(100 + pos), position: pos, name: format!("Tema {pos}"), activities: acts } }
    async fn pool() -> (tempfile::TempDir, SqlitePool) {
        let d = tempfile::tempdir().unwrap();
        let p = db::open(&d.path().join("t.db")).await.unwrap();
        (d, p)
    }

    #[tokio::test]
    async fn courses_are_idempotent_and_never_deleted() {
        let (_d, p) = pool().await;
        let l = vec![course(1, "Desarrollo de Interfaces"), course(2, "Programación")];
        assert_eq!(apply_courses(&p, &l).await.unwrap().changed, 2);
        assert_eq!(apply_courses(&p, &l).await.unwrap().changed, 0, "segunda pasada sin cambios");
        let st = apply_courses(&p, &l[..1]).await.unwrap();
        assert_eq!(st.removed, 1);
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM course").fetch_one(&p).await.unwrap();
        assert_eq!(n, 2, "se marca, no se borra");
        apply_courses(&p, &l).await.unwrap();
        let r: Option<i64> = sqlx::query_scalar("SELECT removed_at FROM course WHERE moodle_id=2").fetch_one(&p).await.unwrap();
        assert!(r.is_none(), "reaparece y se reactiva");
    }

    fn info(cmid: i64, due: Option<i64>, status: &str) -> crate::moodle::mapper::AssignmentInfo {
        crate::moodle::mapper::AssignmentInfo { cmid, opens_at: Some(1000), due_at: due, submission_status: Some(status.into()), grading_status: None, grade: None, feedback: None,
            graded_at: None, grader: None, time_remaining: None, last_modified_at: None, files: vec![], can_submit: true, raw_rows: "{}".into(), ..Default::default() }
    }
    #[tokio::test]
    async fn assignment_sync_is_idempotent_and_logs_date_changes() {
        let (_d, p) = pool().await;
        apply_courses(&p, &[course(1, "C")]).await.unwrap();
        apply_contents(&p, 1, &[sec(0, vec![act(10, "Tarea 1")])]).await.unwrap();
        assert_eq!(apply_assignment(&p, &info(10, Some(5000), "Todavía no se han realizado envíos")).await.unwrap().changed, 1);
        assert_eq!(apply_assignment(&p, &info(10, Some(5000), "Todavía no se han realizado envíos")).await.unwrap().changed, 0);
        sqlx::query("UPDATE activity SET favorite=1, priority=2 WHERE moodle_cmid=10").execute(&p).await.unwrap();
        apply_assignment(&p, &info(10, Some(9000), "Enviado para calificar")).await.unwrap();
        let (due, fav, prio): (i64, i64, i64) = sqlx::query_as("SELECT due_at, favorite, priority FROM activity WHERE moodle_cmid=10").fetch_one(&p).await.unwrap();
        assert_eq!((due, fav, prio), (9000, 1, 2), "la fecha se actualiza y los datos locales se conservan");
        let ch: Vec<String> = sqlx::query_scalar("SELECT message FROM sync_history WHERE level='change'").fetch_all(&p).await.unwrap();
        assert_eq!(ch.len(), 1); assert!(ch[0].starts_with("due_changed|") && ch[0].contains("|5000|9000|"));
        assert!(apply_assignment(&p, &info(999, None, "x")).await.is_err(), "tarea desconocida");
    }

    #[tokio::test]
    async fn first_content_sync_is_silent_but_later_new_items_notify() {
        let (_d, p) = pool().await;
        apply_courses(&p, &[course(1, "C")]).await.unwrap();
        apply_contents(&p, 1, &[sec(0, vec![act(10, "Tarea 1")])]).await.unwrap();
        assert_eq!(crate::notif::list(&p, 10, false).await.unwrap().len(), 0, "lo que ya existía no avisa");
        apply_contents(&p, 1, &[sec(0, vec![act(10, "Tarea 1"), act(11, "Tarea nueva")])]).await.unwrap();
        let n = crate::notif::list(&p, 10, false).await.unwrap();
        assert_eq!(n.len(), 1); assert!(n[0].title.starts_with("Nueva tarea: Tarea nueva"));
        apply_contents(&p, 1, &[sec(0, vec![act(10, "Tarea 1"), act(11, "Tarea nueva")])]).await.unwrap();
        assert_eq!(crate::notif::list(&p, 10, false).await.unwrap().len(), 1, "no se repite");
    }

    #[tokio::test]
    async fn grade_and_date_changes_notify_only_after_first_read() {
        let (_d, p) = pool().await;
        apply_courses(&p, &[course(1, "C")]).await.unwrap();
        apply_contents(&p, 1, &[sec(0, vec![act(10, "Tarea 1")])]).await.unwrap();
        let mut i = info(10, Some(5000), "Todavía no se han realizado envíos"); i.grade = Some("8,00 / 10,00".into());
        apply_assignment(&p, &i).await.unwrap();
        assert_eq!(crate::notif::list(&p, 10, false).await.unwrap().len(), 0, "primera lectura: sin avisos");
        let mut j = info(10, Some(9000), "Enviado para calificar"); j.grade = Some("9,00 / 10,00".into());
        apply_assignment(&p, &j).await.unwrap();
        let t: Vec<String> = crate::notif::list(&p, 10, false).await.unwrap().into_iter().map(|n| n.title).collect();
        assert!(t.iter().any(|x| x.starts_with("Cambio de fecha")) && t.iter().any(|x| x.starts_with("Nueva calificación")), "{t:?}");
        apply_assignment(&p, &j).await.unwrap();
        assert_eq!(crate::notif::list(&p, 10, false).await.unwrap().len(), 2, "sin cambios no hay avisos");
    }

    #[tokio::test]
    async fn folder_names_are_unique_and_stable() {
        let (_d, p) = pool().await;
        apply_courses(&p, &[course(1, "Prog."), course(2, "Prog")]).await.unwrap();
        let f: Vec<String> = sqlx::query_scalar("SELECT folder_name FROM course ORDER BY moodle_id").fetch_all(&p).await.unwrap();
        assert_ne!(f[0], f[1]);
        apply_courses(&p, &[course(1, "Prog. renombrado"), course(2, "Prog")]).await.unwrap();
        let f2: String = sqlx::query_scalar("SELECT folder_name FROM course WHERE moodle_id=1").fetch_one(&p).await.unwrap();
        assert_eq!(f2, f[0], "la carpeta no cambia al renombrar el curso");
    }

    #[tokio::test]
    async fn contents_keep_local_data_and_reject_empty_overwrite() {
        let (_d, p) = pool().await;
        apply_courses(&p, &[course(1, "C")]).await.unwrap();
        let s = vec![sec(0, vec![act(10, "Tarea 1")]), sec(1, vec![act(11, "Tarea 2")])];
        assert_eq!(apply_contents(&p, 1, &s).await.unwrap().changed, 2);
        sqlx::query("UPDATE activity SET favorite=1, local_status='started' WHERE moodle_cmid=10").execute(&p).await.unwrap();
        assert_eq!(apply_contents(&p, 1, &s).await.unwrap().changed, 0);
        let fav: i64 = sqlx::query_scalar("SELECT favorite FROM activity WHERE moodle_cmid=10").fetch_one(&p).await.unwrap();
        assert_eq!(fav, 1, "los datos locales sobreviven a la sincronización");
        // extracción vacía sospechosa: se rechaza y no toca nada
        assert!(apply_contents(&p, 1, &[sec(0, vec![])]).await.is_err());
        let live: i64 = sqlx::query_scalar("SELECT count(*) FROM activity WHERE removed_at IS NULL").fetch_one(&p).await.unwrap();
        assert_eq!(live, 2);
        // una actividad desaparece de verdad: se marca
        let st = apply_contents(&p, 1, &[sec(0, vec![act(10, "Tarea 1")]), sec(1, vec![act(12, "Nueva")])]).await.unwrap();
        assert_eq!(st.removed, 1);
        assert!(apply_contents(&p, 99, &s).await.is_err(), "curso desconocido");
    }
}
