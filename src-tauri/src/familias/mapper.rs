//! Contrato de datos del extractor de Familias (JSON) → modelos validados. Misma filosofía que `moodle::mapper`:
//! una extracción que no cumple el contrato se RECHAZA entera sin tocar la BD; una lista vacía solo vale si el extractor
//! confirma que reconoció el contenedor de la página (`container_found`), para no confundir «sin faltas» con «página rota».
//! El texto del portal es entrada no fiable: se limpia de caracteres de control y se acota.
use crate::error::{AppError, AppResult};
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const SUPPORTED: &str = "f1";
const MAX_ITEMS: usize = 2000;

fn bad(m: impl Into<String>) -> AppError { AppError::Other(format!("extracción de Familias inválida: {}", m.into())) }

/// Texto de una sola línea (o varias si `multiline`), sin caracteres de control y acotado.
pub fn clean(s: &str, max: usize, multiline: bool) -> String {
    let t: String = s.chars().filter(|c| !c.is_control() || (multiline && *c == '\n')).collect();
    let t = if multiline { t.trim().to_string() } else { t.split_whitespace().collect::<Vec<_>>().join(" ") };
    t.chars().take(max).collect()
}

fn opt(s: Option<String>, max: usize) -> Option<String> { s.map(|v| clean(&v, max, false)).filter(|v| !v.is_empty()) }

fn id_ok(id: &str) -> AppResult<String> {
    let id = clean(id, 160, false);
    if id.is_empty() { return Err(bad("elemento sin id")); }
    Ok(id)
}

pub fn hash(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts { h.update(p.as_bytes()); h.update([0x1f]); }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn check_header(version: &str, container_found: bool, n: usize) -> AppResult<()> {
    if version != SUPPORTED { return Err(bad(format!("selectores {version} no soportados"))); }
    if !container_found { return Err(bad("estructura de la página no reconocida")); }
    if n > MAX_ITEMS { return Err(bad("demasiados elementos")); }
    Ok(())
}

fn uniq(ids: impl Iterator<Item = String>) -> AppResult<()> {
    let mut seen = std::collections::HashSet::new();
    for i in ids { if !seen.insert(i) { return Err(bad("id duplicado")); } }
    Ok(())
}

// ---------- alumnado ----------
#[derive(Debug, Deserialize)]
struct StudentsRaw { selector_version: String, #[serde(default)] container_found: bool, students: Vec<StudentRaw> }
#[derive(Debug, Deserialize)]
struct StudentRaw { r#ref: String, name: String, center: Option<String>, group: Option<String>, tutor: Option<String> }
#[derive(Debug, Clone, PartialEq)]
pub struct StudentItem { pub external_ref: String, pub name: String, pub center: Option<String>, pub group: Option<String>, pub tutor: Option<String> }

pub fn students(v: serde_json::Value) -> AppResult<Vec<StudentItem>> {
    let p: StudentsRaw = serde_json::from_value(v).map_err(|e| bad(e.to_string()))?;
    check_header(&p.selector_version, p.container_found, p.students.len())?;
    if p.students.is_empty() { return Err(bad("una cuenta de Familias sin ningún alumno: se rechaza para no retirar a todos")); }
    let out: Vec<StudentItem> = p.students.into_iter().map(|s| {
        let name = clean(&s.name, 160, false);
        if name.is_empty() { return Err(bad("alumno sin nombre")); }
        Ok(StudentItem { external_ref: id_ok(&s.r#ref)?, name, center: opt(s.center, 160), group: opt(s.group, 80), tutor: opt(s.tutor, 160) })
    }).collect::<AppResult<_>>()?;
    uniq(out.iter().map(|s| s.external_ref.clone()))?;
    Ok(out)
}

// ---------- calificaciones ----------
#[derive(Debug, Deserialize)]
struct GradesRaw { selector_version: String, #[serde(default)] container_found: bool, student_ref: String, /// Evaluación que cubre esta extracción: solo se retiran las notas de esa evaluación.
    #[serde(default)] evaluation_scope: Option<String>, items: Vec<GradeRaw> }
#[derive(Debug, Deserialize)]
struct GradeRaw { id: String, subject: String, evaluation: String, grade_text: String, grade_num: Option<f64>, observations: Option<String>, teacher: Option<String>, date: Option<i64> }
#[derive(Debug, Clone, PartialEq)]
pub struct GradeItem { pub id: String, pub subject: String, pub evaluation: String, pub grade_text: String, pub grade_num: Option<f64>, pub observations: Option<String>, pub teacher: Option<String>, pub date: Option<i64>, pub hash: String }

pub fn grades(v: serde_json::Value) -> AppResult<(String, Option<String>, Vec<GradeItem>)> {
    let p: GradesRaw = serde_json::from_value(v).map_err(|e| bad(e.to_string()))?;
    check_header(&p.selector_version, p.container_found, p.items.len())?;
    let out: Vec<GradeItem> = p.items.into_iter().map(|g| {
        let (subject, evaluation, grade_text) = (clean(&g.subject, 160, false), clean(&g.evaluation, 80, false), clean(&g.grade_text, 40, false));
        if subject.is_empty() || evaluation.is_empty() || grade_text.is_empty() { return Err(bad("calificación sin asignatura, evaluación o nota")); }
        if g.grade_num.is_some_and(|n| !n.is_finite() || !(0.0..=100.0).contains(&n)) { return Err(bad("nota numérica fuera de rango")); }
        let (obs, teacher) = (opt(g.observations, 2000), opt(g.teacher, 160));
        let hash = hash(&[&subject, &evaluation, &grade_text, obs.as_deref().unwrap_or(""), teacher.as_deref().unwrap_or("")]);
        Ok(GradeItem { id: id_ok(&g.id)?, subject, evaluation, grade_text, grade_num: g.grade_num, observations: obs, teacher, date: g.date, hash })
    }).collect::<AppResult<_>>()?;
    uniq(out.iter().map(|g| g.id.clone()))?;
    let scope = opt(p.evaluation_scope, 80);
    if let Some(sc) = &scope { if out.iter().any(|g| &g.evaluation != sc) { return Err(bad("nota de otra evaluación dentro de una extracción acotada")); } }
    Ok((id_ok(&p.student_ref)?, scope, out))
}

// ---------- faltas ----------
#[derive(Debug, Deserialize)]
struct AttRaw { selector_version: String, #[serde(default)] container_found: bool, student_ref: String, items: Vec<AttItemRaw> }
#[derive(Debug, Deserialize)]
struct AttItemRaw { id: String, day: String, slot: Option<String>, subject: Option<String>, kind: String, #[serde(default)] justified: bool, justification: Option<String> }
#[derive(Debug, Clone, PartialEq)]
pub struct AttendanceItem { pub id: String, pub day: String, pub slot: Option<String>, pub subject: Option<String>, pub kind: String, pub justified: bool, pub justification: Option<String>, pub hash: String }

pub fn attendance(v: serde_json::Value) -> AppResult<(String, Vec<AttendanceItem>)> {
    let p: AttRaw = serde_json::from_value(v).map_err(|e| bad(e.to_string()))?;
    check_header(&p.selector_version, p.container_found, p.items.len())?;
    let out: Vec<AttendanceItem> = p.items.into_iter().map(|a| {
        chrono::NaiveDate::parse_from_str(&a.day, "%Y-%m-%d").map_err(|_| bad(format!("fecha no válida «{}»", clean(&a.day, 20, false))))?;
        if !["falta", "retraso", "otro"].contains(&a.kind.as_str()) { return Err(bad(format!("tipo de falta desconocido «{}»", clean(&a.kind, 20, false)))); }
        let (slot, subject, just) = (opt(a.slot, 40), opt(a.subject, 160), opt(a.justification, 1000));
        let hash = hash(&[&a.day, &a.kind, slot.as_deref().unwrap_or(""), subject.as_deref().unwrap_or(""), if a.justified { "1" } else { "0" }, just.as_deref().unwrap_or("")]);
        Ok(AttendanceItem { id: id_ok(&a.id)?, day: a.day, slot, subject, kind: a.kind, justified: a.justified, justification: just, hash })
    }).collect::<AppResult<_>>()?;
    uniq(out.iter().map(|a| a.id.clone()))?;
    Ok((id_ok(&p.student_ref)?, out))
}

// ---------- mensajes, avisos, observaciones ----------
#[derive(Debug, Deserialize)]
struct MsgRaw { selector_version: String, #[serde(default)] container_found: bool, student_ref: Option<String>, kind: String, items: Vec<MsgItemRaw> }
#[derive(Debug, Deserialize)]
struct MsgItemRaw { id: String, subject: String, sender: Option<String>, body: Option<String>, sent_at: Option<i64>, #[serde(default)] read: bool }
#[derive(Debug, Clone, PartialEq)]
pub struct MessageItem { pub id: String, pub subject: String, pub sender: Option<String>, pub body: Option<String>, pub sent_at: Option<i64>, pub read: bool, pub hash: String }

pub fn messages(v: serde_json::Value) -> AppResult<(Option<String>, String, Vec<MessageItem>)> {
    let p: MsgRaw = serde_json::from_value(v).map_err(|e| bad(e.to_string()))?;
    check_header(&p.selector_version, p.container_found, p.items.len())?;
    if !["mensaje", "aviso", "observacion", "tutoria"].contains(&p.kind.as_str()) { return Err(bad("tipo de comunicación desconocido")); }
    let out: Vec<MessageItem> = p.items.into_iter().map(|m| {
        let subject = clean(&m.subject, 300, false);
        if subject.is_empty() { return Err(bad("mensaje sin asunto")); }
        let (sender, body) = (opt(m.sender, 160), m.body.map(|b| clean(&b, 20000, true)).filter(|b| !b.is_empty()));
        // El hash NO incluye «leído»: marcarlo como leído en el portal no es un cambio del contenido.
        let hash = hash(&[&subject, sender.as_deref().unwrap_or(""), body.as_deref().unwrap_or(""), &m.sent_at.unwrap_or(0).to_string()]);
        Ok(MessageItem { id: id_ok(&m.id)?, subject, sender, body, sent_at: m.sent_at, read: m.read, hash })
    }).collect::<AppResult<_>>()?;
    uniq(out.iter().map(|m| m.id.clone()))?;
    Ok((p.student_ref.map(|r| id_ok(&r)).transpose()?, p.kind, out))
}

// ---------- tutorías / eventos ----------
#[derive(Debug, Deserialize)]
struct EventsRaw { selector_version: String, #[serde(default)] container_found: bool, student_ref: Option<String>, items: Vec<EventRaw> }
#[derive(Debug, Deserialize)]
struct EventRaw { id: String, title: String, starts_at: i64, ends_at: Option<i64>, #[serde(default)] cancelled: bool }
#[derive(Debug, Clone, PartialEq)]
pub struct EventItem { pub id: String, pub title: String, pub starts_at: i64, pub ends_at: Option<i64>, pub cancelled: bool }

pub fn events(v: serde_json::Value) -> AppResult<(Option<String>, Vec<EventItem>)> {
    let p: EventsRaw = serde_json::from_value(v).map_err(|e| bad(e.to_string()))?;
    check_header(&p.selector_version, p.container_found, p.items.len())?;
    let out: Vec<EventItem> = p.items.into_iter().map(|e| {
        let title = clean(&e.title, 300, false);
        if title.is_empty() || e.starts_at <= 0 { return Err(bad("evento sin título o sin fecha")); }
        if e.ends_at.is_some_and(|x| x < e.starts_at) { return Err(bad("evento que termina antes de empezar")); }
        Ok(EventItem { id: id_ok(&e.id)?, title, starts_at: e.starts_at, ends_at: e.ends_at, cancelled: e.cancelled })
    }).collect::<AppResult<_>>()?;
    uniq(out.iter().map(|e| e.id.clone()))?;
    Ok((p.student_ref.map(|r| id_ok(&r)).transpose()?, out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // PAYLOADS SINTÉTICOS: no proceden del portal real (todavía no hay extractor verificado).
    #[test]
    fn rejects_unknown_version_and_unrecognised_page() {
        assert!(grades(json!({"selector_version":"f9","container_found":true,"student_ref":"a","items":[]})).is_err());
        assert!(grades(json!({"selector_version":"f1","container_found":false,"student_ref":"a","items":[]})).is_err(), "página no reconocida ≠ sin notas");
        assert!(grades(json!({"selector_version":"f1","student_ref":"a","items":[]})).is_err(), "container_found ausente = false");
        let (r, _, items) = grades(json!({"selector_version":"f1","container_found":true,"student_ref":"a","items":[]})).unwrap();
        assert_eq!((r.as_str(), items.len()), ("a", 0), "lista vacía con contenedor reconocido sí vale");
    }

    #[test]
    fn grades_are_validated_cleaned_and_hashed() {
        let ok = json!({"selector_version":"f1","container_found":true,"student_ref":"a","items":[
            {"id":"g1","subject":"  Matemáticas \n","evaluation":"1.ª Eval","grade_text":"7","grade_num":7.0},
            {"id":"g2","subject":"Lengua","evaluation":"1.ª Eval","grade_text":"IN","observations":"Mejorar\u{0007}"}]});
        let (_, _, g) = grades(ok.clone()).unwrap();
        assert_eq!(g[0].subject, "Matemáticas");
        assert_eq!(g[1].observations.as_deref(), Some("Mejorar"), "sin caracteres de control");
        assert_eq!(g[1].grade_num, None, "«IN» no es numérico");
        let (_, _, again) = grades(ok).unwrap();
        assert_eq!(g[0].hash, again[0].hash, "hash estable");
        assert_ne!(g[0].hash, g[1].hash);
        for bad in [
            json!({"id":"","subject":"x","evaluation":"e","grade_text":"5"}),
            json!({"id":"g","subject":"","evaluation":"e","grade_text":"5"}),
            json!({"id":"g","subject":"x","evaluation":"e","grade_text":"5","grade_num":1000.0}),
        ] {
            assert!(grades(json!({"selector_version":"f1","container_found":true,"student_ref":"a","items":[bad]})).is_err());
        }
        let dup = json!({"selector_version":"f1","container_found":true,"student_ref":"a","items":[
            {"id":"g1","subject":"x","evaluation":"e","grade_text":"5"},{"id":"g1","subject":"y","evaluation":"e","grade_text":"6"}]});
        assert!(grades(dup).is_err(), "ids duplicados");
    }

    #[test]
    fn attendance_checks_dates_and_kinds() {
        let mk = |day: &str, kind: &str| json!({"selector_version":"f1","container_found":true,"student_ref":"a","items":[{"id":"1","day":day,"kind":kind,"justified":true}]});
        assert!(attendance(mk("2026-11-03", "falta")).is_ok());
        assert!(attendance(mk("03/11/2026", "falta")).is_err());
        assert!(attendance(mk("2026-02-30", "falta")).is_err());
        assert!(attendance(mk("2026-11-03", "ausencia")).is_err());
    }

    #[test]
    fn message_hash_ignores_read_flag() {
        let mk = |read: bool| json!({"selector_version":"f1","container_found":true,"kind":"mensaje","items":[{"id":"m","subject":"Reunión","body":"Hola","sent_at":100,"read":read}]});
        let (_, _, a) = messages(mk(false)).unwrap();
        let (_, _, b) = messages(mk(true)).unwrap();
        assert_eq!(a[0].hash, b[0].hash);
        assert!(messages(json!({"selector_version":"f1","container_found":true,"kind":"spam","items":[]})).is_err());
    }

    #[test]
    fn a_cuenta_without_students_is_rejected() {
        assert!(students(json!({"selector_version":"f1","container_found":true,"students":[]})).is_err());
        let s = students(json!({"selector_version":"f1","container_found":true,"students":[{"ref":"x","name":"Alumno A","group":"1.º A"}]})).unwrap();
        assert_eq!(s[0].group.as_deref(), Some("1.º A"));
    }

    #[test]
    fn events_validate_order_and_dates() {
        assert!(events(json!({"selector_version":"f1","container_found":true,"items":[{"id":"e","title":"Tutoría","starts_at":100,"ends_at":50}]})).is_err());
        assert!(events(json!({"selector_version":"f1","container_found":true,"items":[{"id":"e","title":"Tutoría","starts_at":0}]})).is_err());
        assert!(events(json!({"selector_version":"f1","container_found":true,"items":[{"id":"e","title":"Tutoría","starts_at":100}]})).is_ok());
    }
}
