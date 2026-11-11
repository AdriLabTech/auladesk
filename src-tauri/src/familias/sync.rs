//! Persistencia de las extracciones de Familias. Mismo patrón que Moodle: idempotente, hash del contenido para detectar
//! cambios, borrado lógico (lo que desaparece del portal se conserva marcado), nada se mezcla entre alumnos.
//! Avisos: la primera extracción de cada tipo y alumno es silenciosa (es la carga inicial, no una novedad), y cada aviso
//! lleva una clave de deduplicación. Un fallo aquí nunca toca las tablas de Moodle.
use super::mapper::*;
use crate::accounts;
use crate::error::AppResult;
use crate::notif::{self, SourcedNotif};
use crate::reminders;
use crate::sync::SyncStats;
use sqlx::SqlitePool;

const SOURCE: &str = "familias";
fn now() -> i64 { chrono::Utc::now().timestamp() }

/// Alumno de la cuenta por su referencia del portal. Una referencia desconocida es un error: no se crean alumnos desde páginas de datos.
async fn student_id(pool: &SqlitePool, account: i64, external_ref: &str) -> AppResult<i64> {
    sqlx::query_scalar("SELECT id FROM student WHERE account_id=? AND external_ref=? AND removed_at IS NULL").bind(account).bind(external_ref).fetch_optional(pool).await?
        .ok_or_else(|| crate::error::AppError::Other("alumno desconocido: sincroniza primero la lista de alumnos".into()))
}

/// ¿Es la primera vez que se recibe este tipo de dato para este alumno? Marca como hecha la carga inicial.
async fn first_load(pool: &SqlitePool, account: i64, student: Option<i64>, kind: &str) -> AppResult<bool> {
    let key = format!("familias_seeded:{account}:{}:{kind}", student.unwrap_or(0));
    let seeded = crate::db::get_setting(pool, &key).await?.is_some();
    if !seeded { crate::db::set_setting(pool, &key, "1").await?; }
    Ok(!seeded)
}

async fn notify(pool: &SqlitePool, account: i64, student: Option<i64>, category: &str, prio: i64, title: &str, body: Option<&str>, key: &str) {
    if !reminders::load_settings(pool).await.enabled(category) { return; }
    let n = SourcedNotif { category, priority: prio, title, body, source: SOURCE, account_id: account, student_id: student, dedupe_key: key };
    if let Err(e) = notif::add_sourced(pool, &n).await { tracing::warn!("aviso de Familias: {e}"); }
}

async fn student_name(pool: &SqlitePool, id: i64) -> String {
    sqlx::query_scalar::<_, String>("SELECT display_name FROM student WHERE id=?").bind(id).fetch_one(pool).await.unwrap_or_default()
}

pub async fn apply_students(pool: &SqlitePool, account: i64, items: &[StudentItem]) -> AppResult<SyncStats> {
    let mut st = SyncStats { seen: items.len() as i64, ..Default::default() };
    for s in items {
        let before: Option<(String,)> = sqlx::query_as("SELECT display_name FROM student WHERE account_id=? AND external_ref=?").bind(account).bind(&s.external_ref).fetch_optional(pool).await?;
        if before.as_ref().map_or(true, |(n,)| *n != s.name) { st.changed += 1; }
        accounts::upsert_student(pool, account, &accounts::NewStudent { external_ref: &s.external_ref, display_name: &s.name, center: s.center.as_deref(), course_group: s.group.as_deref(), tutor: s.tutor.as_deref() }).await?;
    }
    let keep: Vec<String> = items.iter().map(|s| s.external_ref.clone()).collect();
    st.removed = accounts::retire_missing(pool, account, &keep).await?;
    Ok(st)
}

pub async fn apply_grades(pool: &SqlitePool, account: i64, student_ref: &str, scope: Option<&str>, items: &[GradeItem]) -> AppResult<SyncStats> {
    let sid = student_id(pool, account, student_ref).await?;
    let first = first_load(pool, account, Some(sid), "grades").await?;
    let t = now();
    let mut st = SyncStats { seen: items.len() as i64, ..Default::default() };
    let mut tx = pool.begin().await?;
    let mut news = vec![];
    for g in items {
        let old: Option<(i64, String, Option<i64>)> = sqlx::query_as("SELECT id, content_hash, removed_at FROM grade_record WHERE student_id=? AND external_id=?").bind(sid).bind(&g.id).fetch_optional(&mut *tx).await?;
        match old {
            None => {
                sqlx::query("INSERT INTO grade_record(student_id,external_id,subject,evaluation,grade_text,grade_num,observations,teacher,recorded_at,content_hash,first_seen_at,last_synced_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)")
                    .bind(sid).bind(&g.id).bind(&g.subject).bind(&g.evaluation).bind(&g.grade_text).bind(g.grade_num).bind(&g.observations).bind(&g.teacher).bind(g.date).bind(&g.hash).bind(t).bind(t).execute(&mut *tx).await?;
                st.changed += 1; news.push((g, false));
            }
            Some((id, h, removed)) => {
                if h != g.hash || removed.is_some() {
                    sqlx::query("UPDATE grade_record SET subject=?,evaluation=?,grade_text=?,grade_num=?,observations=?,teacher=?,recorded_at=?,content_hash=?,last_synced_at=?,removed_at=NULL WHERE id=?")
                        .bind(&g.subject).bind(&g.evaluation).bind(&g.grade_text).bind(g.grade_num).bind(&g.observations).bind(&g.teacher).bind(g.date).bind(&g.hash).bind(t).bind(id).execute(&mut *tx).await?;
                    st.changed += 1; if h != g.hash { news.push((g, true)); }
                } else { sqlx::query("UPDATE grade_record SET last_synced_at=? WHERE id=?").bind(t).bind(id).execute(&mut *tx).await?; }
            }
        }
    }
    let keep: Vec<&str> = items.iter().map(|g| g.id.as_str()).collect();
    // Con extracción acotada a una evaluación, solo se retiran las notas de ESA evaluación (las demás no se tocan).
    let current: Vec<(i64, String)> = sqlx::query_as("SELECT id, external_id FROM grade_record WHERE student_id=? AND removed_at IS NULL AND (?2 IS NULL OR evaluation=?2)").bind(sid).bind(scope).fetch_all(&mut *tx).await?;
    for (id, ext) in current.into_iter().filter(|(_, e)| !keep.contains(&e.as_str())) {
        let _ = ext;
        sqlx::query("UPDATE grade_record SET removed_at=? WHERE id=?").bind(t).bind(id).execute(&mut *tx).await?;
        st.removed += 1;
    }
    tx.commit().await?;
    if !first {
        let who = student_name(pool, sid).await;
        for (g, changed) in news {
            let title = format!("{} · {}: {}", if changed { "Calificación modificada" } else { "Nueva calificación" }, g.subject, g.grade_text);
            notify(pool, account, Some(sid), "family_grade", 1, &title, Some(&format!("{who} · {}", g.evaluation)), &format!("familias:{sid}:grade:{}:{}", g.id, &g.hash[..12])).await;
        }
    }
    Ok(st)
}

pub async fn apply_attendance(pool: &SqlitePool, account: i64, student_ref: &str, items: &[AttendanceItem]) -> AppResult<SyncStats> {
    let sid = student_id(pool, account, student_ref).await?;
    let first = first_load(pool, account, Some(sid), "attendance").await?;
    let t = now();
    let mut st = SyncStats { seen: items.len() as i64, ..Default::default() };
    let mut tx = pool.begin().await?;
    let mut news = vec![];
    for a in items {
        let old: Option<(i64, String, Option<i64>)> = sqlx::query_as("SELECT id, content_hash, removed_at FROM attendance_record WHERE student_id=? AND external_id=?").bind(sid).bind(&a.id).fetch_optional(&mut *tx).await?;
        match old {
            None => {
                sqlx::query("INSERT INTO attendance_record(student_id,external_id,day,slot,subject,kind,justified,justification,content_hash,first_seen_at,last_synced_at) VALUES(?,?,?,?,?,?,?,?,?,?,?)")
                    .bind(sid).bind(&a.id).bind(&a.day).bind(&a.slot).bind(&a.subject).bind(&a.kind).bind(a.justified as i64).bind(&a.justification).bind(&a.hash).bind(t).bind(t).execute(&mut *tx).await?;
                st.changed += 1; news.push(a);
            }
            Some((id, h, removed)) => {
                if h != a.hash || removed.is_some() {
                    sqlx::query("UPDATE attendance_record SET day=?,slot=?,subject=?,kind=?,justified=?,justification=?,content_hash=?,last_synced_at=?,removed_at=NULL WHERE id=?")
                        .bind(&a.day).bind(&a.slot).bind(&a.subject).bind(&a.kind).bind(a.justified as i64).bind(&a.justification).bind(&a.hash).bind(t).bind(id).execute(&mut *tx).await?;
                    st.changed += 1;   // un cambio de justificación se registra pero no genera aviso
                } else { sqlx::query("UPDATE attendance_record SET last_synced_at=? WHERE id=?").bind(t).bind(id).execute(&mut *tx).await?; }
            }
        }
    }
    let keep: Vec<&str> = items.iter().map(|a| a.id.as_str()).collect();
    let current: Vec<(i64, String)> = sqlx::query_as("SELECT id, external_id FROM attendance_record WHERE student_id=? AND removed_at IS NULL").bind(sid).fetch_all(&mut *tx).await?;
    for (id, _) in current.into_iter().filter(|(_, e)| !keep.contains(&e.as_str())) {
        sqlx::query("UPDATE attendance_record SET removed_at=? WHERE id=?").bind(t).bind(id).execute(&mut *tx).await?;
        st.removed += 1;
    }
    tx.commit().await?;
    if !first {
        let who = student_name(pool, sid).await;
        for a in news {
            let title = format!("{} el {}{}", if a.kind == "retraso" { "Retraso" } else { "Falta" }, a.day, a.subject.as_deref().map(|s| format!(" · {s}")).unwrap_or_default());
            notify(pool, account, Some(sid), "attendance", 2, &title, Some(&who), &format!("familias:{sid}:att:{}", a.id)).await;
        }
    }
    Ok(st)
}

pub async fn apply_messages(pool: &SqlitePool, account: i64, student_ref: Option<&str>, kind: &str, items: &[MessageItem]) -> AppResult<SyncStats> {
    let sid = match student_ref { Some(r) => Some(student_id(pool, account, r).await?), None => None };
    let first = first_load(pool, account, sid, kind).await?;
    let t = now();
    let mut st = SyncStats { seen: items.len() as i64, ..Default::default() };
    let mut tx = pool.begin().await?;
    let mut news = vec![];
    for m in items {
        let old: Option<(i64, String, Option<i64>)> = sqlx::query_as("SELECT id, content_hash, removed_at FROM school_message WHERE account_id=? AND coalesce(student_id,0)=coalesce(?,0) AND kind=? AND external_id=?").bind(account).bind(sid).bind(kind).bind(&m.id).fetch_optional(&mut *tx).await?;
        match old {
            None => {
                sqlx::query("INSERT INTO school_message(account_id,student_id,external_id,kind,subject,sender,body,sent_at,read_in_portal,content_hash,first_seen_at,last_synced_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)")
                    .bind(account).bind(sid).bind(&m.id).bind(kind).bind(&m.subject).bind(&m.sender).bind(&m.body).bind(m.sent_at).bind(m.read as i64).bind(&m.hash).bind(t).bind(t).execute(&mut *tx).await?;
                st.changed += 1; news.push(m);
            }
            Some((id, h, removed)) => {
                if h != m.hash || removed.is_some() { st.changed += 1; }
                sqlx::query("UPDATE school_message SET subject=?,sender=?,body=?,sent_at=?,read_in_portal=?,content_hash=?,last_synced_at=?,removed_at=NULL WHERE id=?")
                    .bind(&m.subject).bind(&m.sender).bind(&m.body).bind(m.sent_at).bind(m.read as i64).bind(&m.hash).bind(t).bind(id).execute(&mut *tx).await?;
            }
        }
    }
    let keep: Vec<&str> = items.iter().map(|m| m.id.as_str()).collect();
    let current: Vec<(i64, String)> = sqlx::query_as("SELECT id, external_id FROM school_message WHERE account_id=? AND coalesce(student_id,0)=coalesce(?,0) AND kind=? AND removed_at IS NULL").bind(account).bind(sid).bind(kind).fetch_all(&mut *tx).await?;
    for (id, _) in current.into_iter().filter(|(_, e)| !keep.contains(&e.as_str())) {
        sqlx::query("UPDATE school_message SET removed_at=? WHERE id=?").bind(t).bind(id).execute(&mut *tx).await?;
        st.removed += 1;
    }
    tx.commit().await?;
    if !first {
        for m in news.into_iter().filter(|m| !m.read) {
            let title = format!("{}: {}", match kind { "aviso" => "Aviso del centro", "observacion" => "Observación", "tutoria" => "Tutoría", _ => "Mensaje" }, m.subject);
            notify(pool, account, sid, "message", 1, &title, m.sender.as_deref(), &format!("familias:{}:{}:{}:{}", sid.unwrap_or(0), kind, account, m.id)).await;
        }
    }
    Ok(st)
}

/// Tutorías y eventos del portal → calendario unificado (`source='familias'`). El id externo evita duplicados por alumno.
pub async fn apply_events(pool: &SqlitePool, account: i64, student_ref: Option<&str>, items: &[EventItem]) -> AppResult<SyncStats> {
    let sid = match student_ref { Some(r) => Some(student_id(pool, account, r).await?), None => None };
    let first = first_load(pool, account, sid, "events").await?;
    let t = now();
    let mut st = SyncStats { seen: items.len() as i64, ..Default::default() };
    let mut tx = pool.begin().await?;
    let mut news = vec![];
    for e in items {
        let old: Option<(i64, i64, Option<i64>)> = sqlx::query_as("SELECT id, starts_at, cancelled_at FROM calendar_event WHERE source=? AND account_id=? AND coalesce(student_id,0)=coalesce(?,0) AND external_id=?").bind(SOURCE).bind(account).bind(sid).bind(&e.id).fetch_optional(&mut *tx).await?;
        let cancelled = e.cancelled.then_some(t);
        match old {
            None => {
                sqlx::query("INSERT INTO calendar_event(source,account_id,student_id,external_id,kind,title,starts_at,ends_at,all_day,cancelled_at,last_synced_at) VALUES(?,?,?,?,'tutoring',?,?,?,0,?,?)")
                    .bind(SOURCE).bind(account).bind(sid).bind(&e.id).bind(&e.title).bind(e.starts_at).bind(e.ends_at).bind(cancelled).bind(t).execute(&mut *tx).await?;
                st.changed += 1; news.push((e, false));
            }
            Some((id, starts, was_cancelled)) => {
                if starts != e.starts_at || was_cancelled.is_some() != e.cancelled { st.changed += 1; news.push((e, true)); }
                sqlx::query("UPDATE calendar_event SET title=?, starts_at=?, ends_at=?, cancelled_at=?, last_synced_at=? WHERE id=?").bind(&e.title).bind(e.starts_at).bind(e.ends_at).bind(cancelled.or(was_cancelled.filter(|_| e.cancelled))).bind(t).bind(id).execute(&mut *tx).await?;
            }
        }
    }
    tx.commit().await?;
    if !first {
        for (e, changed) in news {
            let title = format!("{}: {}", if changed { "Cambio en la tutoría" } else { "Nueva tutoría" }, e.title);
            notify(pool, account, sid, "tutoring", 1, &title, Some(&reminders::fmt_dt(e.starts_at)), &format!("familias:{}:ev:{}:{}", sid.unwrap_or(0), e.id, e.starts_at)).await;
        }
    }
    Ok(st)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::tests::setup;
    use serde_json::json;

    // DATOS SINTÉTICOS (alumnos ficticios).
    fn grades_payload(student: &str, items: serde_json::Value) -> serde_json::Value { json!({"selector_version":"f1","container_found":true,"student_ref":student,"items":items}) }
    async fn n(p: &SqlitePool, sql: &str) -> i64 { sqlx::query_scalar(sql).fetch_one(p).await.unwrap() }

    #[tokio::test]
    async fn grades_first_load_is_silent_then_news_notify_once() {
        let (_d, p, acc, a, _) = setup().await;
        let g1 = json!({"id":"g1","subject":"Mates","evaluation":"1.ª","grade_text":"7","grade_num":7.0});
        let (r, _, items) = grades(grades_payload("ref-a", json!([g1.clone()]))).unwrap();
        let s = apply_grades(&p, acc, &r, None, &items).await.unwrap();
        assert_eq!((s.seen, s.changed), (1, 1));
        assert_eq!(n(&p, "SELECT count(*) FROM notification").await, 0, "la carga inicial no avisa");
        // idempotente
        let s = apply_grades(&p, acc, &r, None, &items).await.unwrap();
        assert_eq!(s.changed, 0);
        // llega una nota nueva → un aviso; repetir no duplica
        let g2 = json!({"id":"g2","subject":"Lengua","evaluation":"1.ª","grade_text":"5","grade_num":5.0});
        let (_, _, items2) = grades(grades_payload("ref-a", json!([g1.clone(), g2.clone()]))).unwrap();
        apply_grades(&p, acc, &r, None, &items2).await.unwrap();
        apply_grades(&p, acc, &r, None, &items2).await.unwrap();
        assert_eq!(n(&p, "SELECT count(*) FROM notification WHERE category='family_grade'").await, 1);
        let (src, sid): (String, i64) = sqlx::query_as("SELECT source, student_id FROM notification").fetch_one(&p).await.unwrap();
        assert_eq!((src.as_str(), sid), ("familias", a));
        // la nota cambia → «modificada», con otra clave
        let g2b = json!({"id":"g2","subject":"Lengua","evaluation":"1.ª","grade_text":"6","grade_num":6.0});
        let (_, _, items3) = grades(grades_payload("ref-a", json!([g1, g2b]))).unwrap();
        assert_eq!(apply_grades(&p, acc, &r, None, &items3).await.unwrap().changed, 1);
        assert_eq!(n(&p, "SELECT count(*) FROM notification WHERE title LIKE 'Calificación modificada%'").await, 1);
    }

    #[tokio::test]
    async fn missing_items_are_soft_deleted_and_can_return() {
        let (_d, p, acc, _, _) = setup().await;
        let mk = |ids: &[&str]| grades(grades_payload("ref-a", json!(ids.iter().map(|i| json!({"id":i,"subject":"S","evaluation":"E","grade_text":"5"})).collect::<Vec<_>>()))).unwrap();
        let (r, _, a) = mk(&["1", "2"]); apply_grades(&p, acc, &r, None, &a).await.unwrap();
        let (r, _, b) = mk(&["1"]); let s = apply_grades(&p, acc, &r, None, &b).await.unwrap();
        assert_eq!(s.removed, 1);
        assert_eq!(n(&p, "SELECT count(*) FROM grade_record").await, 2, "se conserva");
        assert_eq!(n(&p, "SELECT count(*) FROM grade_record WHERE removed_at IS NOT NULL").await, 1);
        apply_grades(&p, acc, &r, None, &a).await.unwrap();
        assert_eq!(n(&p, "SELECT count(*) FROM grade_record WHERE removed_at IS NOT NULL").await, 0);
    }

    #[tokio::test]
    async fn an_evaluation_scoped_extraction_never_retires_other_evaluations() {
        let (_d, p, acc, _, _) = setup().await;
        let mk = |ev: &str, id: &str| grades(grades_payload("ref-a", json!([{"id":id,"subject":"S","evaluation":ev,"grade_text":"5"}])).as_object().map(|o| { let mut o = o.clone(); o.insert("evaluation_scope".into(), json!(ev)); serde_json::Value::Object(o) }).unwrap()).unwrap();
        let (r, sc, a) = mk("1.ª", "a"); apply_grades(&p, acc, &r, sc.as_deref(), &a).await.unwrap();
        let (r, sc, b) = mk("2.ª", "b"); apply_grades(&p, acc, &r, sc.as_deref(), &b).await.unwrap();
        assert_eq!(n(&p, "SELECT count(*) FROM grade_record WHERE removed_at IS NULL").await, 2, "la 2.ª no retira la 1.ª");
        // la 1.ª ahora vacía (con contenedor reconocido): solo se retira la de la 1.ª
        let empty = grades(json!({"selector_version":"f1","container_found":true,"student_ref":"ref-a","evaluation_scope":"1.ª","items":[]})).unwrap();
        apply_grades(&p, acc, &empty.0, empty.1.as_deref(), &empty.2).await.unwrap();
        assert_eq!(n(&p, "SELECT count(*) FROM grade_record WHERE removed_at IS NULL").await, 1);
        let ev: String = sqlx::query_scalar("SELECT evaluation FROM grade_record WHERE removed_at IS NULL").fetch_one(&p).await.unwrap();
        assert_eq!(ev, "2.ª");
        // una nota de otra evaluación dentro de una extracción acotada se rechaza
        assert!(grades(json!({"selector_version":"f1","container_found":true,"student_ref":"a","evaluation_scope":"1.ª","items":[{"id":"x","subject":"S","evaluation":"2.ª","grade_text":"5"}]})).is_err());
    }

    #[tokio::test]
    async fn students_never_mix() {
        let (_d, p, acc, a, b) = setup().await;
        let one = |ext: &str, subj: &str| attendance(json!({"selector_version":"f1","container_found":true,"student_ref":ext,"items":[{"id":"x1","day":"2026-11-03","kind":"falta","subject":subj}]})).unwrap();
        let (ra, ia) = one("ref-a", "Mates"); apply_attendance(&p, acc, &ra, &ia).await.unwrap();
        let (rb, ib) = one("ref-b", "Lengua"); apply_attendance(&p, acc, &rb, &ib).await.unwrap();
        assert_eq!(n(&p, "SELECT count(*) FROM attendance_record").await, 2, "mismo id externo en dos alumnos = dos filas");
        let sa: String = sqlx::query_scalar("SELECT subject FROM attendance_record WHERE student_id=?").bind(a).fetch_one(&p).await.unwrap();
        let sb: String = sqlx::query_scalar("SELECT subject FROM attendance_record WHERE student_id=?").bind(b).fetch_one(&p).await.unwrap();
        assert_eq!((sa.as_str(), sb.as_str()), ("Mates", "Lengua"));
        // quitar la falta del alumno A no toca la del B
        let (ra, empty) = attendance(json!({"selector_version":"f1","container_found":true,"student_ref":"ref-a","items":[]})).unwrap();
        apply_attendance(&p, acc, &ra, &empty).await.unwrap();
        assert_eq!(n(&p, "SELECT count(*) FROM attendance_record WHERE removed_at IS NULL").await, 1);
        // un alumno que no es de la cuenta se rechaza
        let (rx, ix) = one("ref-x", "?");
        assert!(apply_attendance(&p, acc, &rx, &ix).await.is_err());
    }

    #[tokio::test]
    async fn first_absence_after_an_empty_first_load_does_notify() {
        let (_d, p, acc, _, _) = setup().await;
        let (r, none) = attendance(json!({"selector_version":"f1","container_found":true,"student_ref":"ref-a","items":[]})).unwrap();
        apply_attendance(&p, acc, &r, &none).await.unwrap();   // carga inicial vacía
        let (r, one) = attendance(json!({"selector_version":"f1","container_found":true,"student_ref":"ref-a","items":[{"id":"1","day":"2026-11-03","kind":"retraso","subject":"Mates"}]})).unwrap();
        apply_attendance(&p, acc, &r, &one).await.unwrap();
        assert_eq!(n(&p, "SELECT count(*) FROM notification WHERE category='attendance'").await, 1);
    }

    #[tokio::test]
    async fn messages_notify_only_unread_news_and_events_dedupe() {
        let (_d, p, acc, _, _) = setup().await;
        let mk = |ids: &[(&str, bool)]| messages(json!({"selector_version":"f1","container_found":true,"student_ref":"ref-a","kind":"mensaje","items": ids.iter().map(|(i, r)| json!({"id":i,"subject":format!("Asunto {i}"),"read":r})).collect::<Vec<_>>()})).unwrap();
        let (sr, k, a) = mk(&[("1", true)]); apply_messages(&p, acc, sr.as_deref(), &k, &a).await.unwrap();
        let (sr, k, b) = mk(&[("1", true), ("2", false), ("3", true)]); apply_messages(&p, acc, sr.as_deref(), &k, &b).await.unwrap();
        assert_eq!(n(&p, "SELECT count(*) FROM notification WHERE category='message'").await, 1, "solo el no leído");
        assert_eq!(n(&p, "SELECT count(*) FROM school_message").await, 3);
        let ev = |ts: i64| events(json!({"selector_version":"f1","container_found":true,"student_ref":"ref-a","items":[{"id":"t1","title":"Tutoría","starts_at":ts}]})).unwrap();
        let (sr, e0) = ev(1000); apply_events(&p, acc, sr.as_deref(), &e0).await.unwrap();
        let (sr, e1) = ev(2000); assert_eq!(apply_events(&p, acc, sr.as_deref(), &e1).await.unwrap().changed, 1);
        assert_eq!(n(&p, "SELECT count(*) FROM calendar_event WHERE source='familias'").await, 1, "mismo id externo: se actualiza, no se duplica");
        assert_eq!(n(&p, "SELECT starts_at FROM calendar_event WHERE source='familias'").await, 2000);
        assert_eq!(n(&p, "SELECT count(*) FROM notification WHERE category='tutoring'").await, 1);
    }

    #[tokio::test]
    async fn students_apply_retires_missing() {
        let (_d, p, acc, _, _) = setup().await;
        let it = students(json!({"selector_version":"f1","container_found":true,"students":[{"ref":"ref-a","name":"Alumno A"}]})).unwrap();
        let s = apply_students(&p, acc, &it).await.unwrap();
        assert_eq!((s.seen, s.removed), (1, 1));
        assert_eq!(accounts::students(&p, acc).await.unwrap().len(), 1);
    }
}
