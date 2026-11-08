//! MoodleDataMapper: payload del extractor (JSON) → modelos internos, con validación de estructura.
use super::models::*;
use crate::error::{AppError, AppResult};
use serde::Deserialize;

pub const SUPPORTED: &str = "v1";

#[derive(Debug, Deserialize)]
pub struct CoursesPayload { pub selector_version: String, pub total_count: Option<i64>, #[serde(default)] pub more_pages: bool, pub courses: Vec<CourseItem> }
#[derive(Debug, Deserialize)]
pub struct CourseItem { pub moodle_id: i64, pub name: String, pub url: Option<String> }

#[derive(Debug, Deserialize)]
pub struct ContentsPayload { pub selector_version: String, pub course_id: Option<i64>, pub sections: Vec<SectionItem> }
#[derive(Debug, Deserialize)]
pub struct SectionItem { pub moodle_id: Option<i64>, pub position: i64, pub name: String, pub activities: Vec<ActivityItem> }
#[derive(Debug, Deserialize)]
pub struct ActivityItem { pub cmid: i64, pub modname: String, pub name: String, pub url: Option<String>, pub description: Option<String> }

fn bad(m: impl Into<String>) -> AppError { AppError::Other(format!("extracción inválida: {}", m.into())) }

pub fn courses(v: serde_json::Value) -> AppResult<Vec<RemoteCourse>> {
    let p: CoursesPayload = serde_json::from_value(v).map_err(|e| bad(e.to_string()))?;
    if p.selector_version != SUPPORTED { return Err(bad(format!("selectores {} no soportados", p.selector_version))); }
    if p.more_pages { return Err(bad("la lista tiene más páginas; extracción incompleta")); }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for c in p.courses {
        if c.moodle_id <= 0 || c.name.trim().is_empty() { return Err(bad("curso sin id o sin nombre")); }
        if !seen.insert(c.moodle_id) { return Err(bad("id de curso duplicado")); }
        out.push(RemoteCourse { moodle_id: c.moodle_id, fullname: c.name.trim().to_string(), shortname: None, url: c.url.unwrap_or_default() });
    }
    Ok(out)
}

pub fn contents(v: serde_json::Value) -> AppResult<(i64, Vec<RemoteSection>)> {
    let p: ContentsPayload = serde_json::from_value(v).map_err(|e| bad(e.to_string()))?;
    if p.selector_version != SUPPORTED { return Err(bad(format!("selectores {} no soportados", p.selector_version))); }
    let course = p.course_id.filter(|i| *i > 0).ok_or_else(|| bad("sin id de curso"))?;
    if p.sections.is_empty() { return Err(bad("curso sin secciones")); }
    let mut cmids = std::collections::HashSet::new();
    let mut out = Vec::new();
    for s in p.sections {
        let mut acts = Vec::new();
        for a in s.activities {
            if a.cmid <= 0 { return Err(bad("actividad sin cmid")); }
            if !cmids.insert(a.cmid) { return Err(bad("cmid duplicado")); }
            acts.push(RemoteActivity { cmid: a.cmid, modname: a.modname, name: a.name, url: a.url, due_at: None, files: vec![] });
        }
        out.push(RemoteSection { moodle_id: s.moodle_id, position: s.position, name: s.name, activities: acts });
    }
    Ok((course, out))
}

#[derive(Debug, Deserialize)]
pub struct FilesPayload { pub selector_version: String, pub files: Vec<FileItem> }
#[derive(Debug, Deserialize)]
pub struct FileItem { pub name: Option<String>, pub url: String }

/// Solo archivos que publica el profesorado en recursos/carpetas/enunciados. Se excluyen entregas propias y comentarios.
pub fn downloadable(url: &str) -> bool {
    let Ok(u) = tauri::Url::parse(url) else { return false };
    if !super::window::is_allowed(&u) { return false; }
    let segs: Vec<&str> = u.path_segments().map(|s| s.collect()).unwrap_or_default();
    let Some(i) = segs.iter().position(|s| *s == "pluginfile.php") else { return false };
    match (segs.get(i + 2), segs.get(i + 3)) {
        (Some(&"mod_resource"), Some(&"content")) | (Some(&"mod_folder"), Some(&"content")) => true,
        (Some(&"mod_assign"), Some(&"intro")) | (Some(&"mod_assign"), Some(&"introattachment")) => true,
        _ => false,
    }
}

pub fn files(v: serde_json::Value) -> AppResult<Vec<(String, String)>> {
    let p: FilesPayload = serde_json::from_value(v).map_err(|e| bad(e.to_string()))?;
    if p.selector_version != SUPPORTED { return Err(bad(format!("selectores {} no soportados", p.selector_version))); }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for f in p.files {
        let clean = f.url.split('#').next().unwrap_or("").to_string();
        if downloadable(&clean) && seen.insert(clean.clone()) { out.push((f.name.unwrap_or_default(), clean)); }
    }
    Ok(out.into_iter().take(60).collect())
}

// ---------------- fechas de otras actividades y calendario ----------------
#[derive(Debug, Deserialize)]
pub struct DatesPayload { pub selector_version: String, pub cmid: Option<i64>, pub modname: Option<String>, pub error: Option<String>, #[serde(default)] pub dates: Vec<String> }

/// (apertura, cierre) en segundos UTC. Soporta «Apertura/Cierre» (tareas) y «Abrió/Cerró/Se abrirá/Se cerrará» (cuestionarios).
pub fn dates_from_lines(lines: &[String]) -> (Option<i64>, Option<i64>) {
    use super::dates::{parse_es_datetime as pd, DEFAULT_TZ as TZ};
    let pick = |keys: &[&str]| lines.iter().find_map(|l| {
        let low = l.to_lowercase();
        keys.iter().find(|k| low.starts_with(*k)).and_then(|_| l.split_once(':').and_then(|(_, rest)| pd(rest, TZ)))
    });
    (pick(&["apertura", "abrió", "se abrirá", "abierto"]), pick(&["cierre", "cerró", "se cerrará", "cerrado", "fecha de entrega", "vencimiento"]))
}

pub fn activity_dates(v: serde_json::Value) -> AppResult<(i64, String, Option<i64>, Option<i64>)> {
    let p: DatesPayload = serde_json::from_value(v).map_err(|e| bad(e.to_string()))?;
    if p.selector_version != SUPPORTED { return Err(bad(format!("selectores {} no soportados", p.selector_version))); }
    if let Some(e) = p.error { return Err(bad(format!("Moodle devolvió una página de error: {e}"))); }
    let cmid = p.cmid.filter(|c| *c > 0).ok_or_else(|| bad("sin cmid"))?;
    let (o, c) = dates_from_lines(&p.dates);
    Ok((cmid, p.modname.unwrap_or_default(), o, c))
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageEvent { pub moodle_id: i64, pub title: String, pub component: String, pub eventtype: String, pub course_id: Option<i64>, pub starts_at: i64, pub cmid: Option<i64> }

#[derive(Debug, Deserialize)]
struct CalPayload { selector_version: String, events: Vec<CalItem> }
#[derive(Debug, Deserialize)]
struct CalItem { id: i64, title: String, #[serde(default)] component: String, #[serde(default)] eventtype: String, course_id: Option<i64>, timestamp: Option<i64>, url: Option<String> }

pub fn calendar_events(v: serde_json::Value) -> AppResult<Vec<PageEvent>> {
    let p: CalPayload = serde_json::from_value(v).map_err(|e| bad(e.to_string()))?;
    if p.selector_version != SUPPORTED { return Err(bad(format!("selectores {} no soportados", p.selector_version))); }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for e in p.events {
        // Un evento sin fecha o sin id no se inventa ni se guarda: se rechaza toda la extracción (estructura rota).
        let ts = e.timestamp.filter(|t| *t > 0).ok_or_else(|| bad("evento sin fecha"))?;
        if e.id <= 0 || e.title.trim().is_empty() { return Err(bad("evento sin id o sin título")); }
        if !seen.insert(e.id) { continue; }
        let cmid = e.url.as_deref().and_then(|u| u.split("id=").nth(1)).and_then(|r| r.split('&').next()).and_then(|n| n.parse().ok());
        out.push(PageEvent { moodle_id: e.id, title: e.title.trim().to_string(), component: e.component, eventtype: e.eventtype, course_id: e.course_id, starts_at: ts, cmid });
    }
    Ok(out)
}

// ---------------- página de una tarea ----------------
#[derive(Debug, Deserialize)]
pub struct Row { pub label: String, pub value: String }
#[derive(Debug, Deserialize)]
pub struct SubFile { pub name: String, pub url: String, pub time_text: Option<String> }
#[derive(Debug, Deserialize)]
pub struct AssignmentPayload {
    pub selector_version: String, pub cmid: Option<i64>, pub page_title: Option<String>, pub error: Option<String>,
    #[serde(default)] pub dates: Vec<String>, #[serde(default)] pub status_rows: Vec<Row>, #[serde(default)] pub grade_rows: Vec<Row>,
    #[serde(default)] pub files: Vec<SubFile>, #[serde(default)] pub can_add: bool, #[serde(default)] pub can_edit: bool,
    #[serde(default)] pub intro_text: Option<String>, #[serde(default)] pub intro_links: Vec<IntroLink>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, serde::Serialize)]
pub struct IntroLink { pub name: String, pub url: String, #[serde(default)] pub file: bool }

#[derive(Debug, Clone, PartialEq, serde::Serialize, Deserialize)]
pub struct SubmittedFile { pub name: String, pub url: String, pub modified_at: Option<i64> }

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AssignmentInfo {
    pub cmid: i64, pub opens_at: Option<i64>, pub due_at: Option<i64>,
    pub submission_status: Option<String>, pub grading_status: Option<String>, pub grade: Option<String>, pub feedback: Option<String>,
    pub graded_at: Option<i64>, pub grader: Option<String>, pub time_remaining: Option<String>, pub last_modified_at: Option<i64>,
    pub files: Vec<SubmittedFile>, pub can_submit: bool, pub raw_rows: String,
    pub intro_text: Option<String>, pub intro_links: Vec<IntroLink>,
}

/// Texto de la página: sin caracteres de control (salvo saltos de línea) y acotado. Es dato no fiable: solo se muestra como texto.
pub fn clean_text(raw: &str, max: usize) -> Option<String> {
    let s: String = raw.chars().filter(|c| !c.is_control() || *c == '\n').collect::<String>().trim().to_string();
    if s.is_empty() { None } else { Some(s.chars().take(max).collect()) }
}

/// Solo enlaces https; nombre acotado. El resto se descarta.
pub fn clean_links(links: &[IntroLink]) -> Vec<IntroLink> {
    let mut seen = std::collections::HashSet::new();
    links.iter().filter(|l| l.url.starts_with("https://") && l.url.len() < 2000 && seen.insert(l.url.clone()))
        .take(60).map(|l| IntroLink { name: clean_text(&l.name, 200).unwrap_or_else(|| l.url.clone()), url: l.url.clone(), file: l.file }).collect()
}

fn find<'a>(rows: &'a [Row], needle: &str) -> Option<&'a str> {
    rows.iter().find(|r| r.label.to_lowercase().contains(needle)).map(|r| r.value.trim()).filter(|v| !v.is_empty() && *v != "-")
}

pub fn assignment(v: serde_json::Value) -> AppResult<AssignmentInfo> {
    use super::dates::{parse_es_datetime as pd, DEFAULT_TZ as TZ};
    let p: AssignmentPayload = serde_json::from_value(v).map_err(|e| bad(e.to_string()))?;
    if p.selector_version != SUPPORTED { return Err(bad(format!("selectores {} no soportados", p.selector_version))); }
    if let Some(e) = p.error { return Err(bad(format!("Moodle devolvió una página de error: {e}"))); }
    let cmid = p.cmid.filter(|c| *c > 0).ok_or_else(|| bad("sin cmid"))?;
    if p.status_rows.is_empty() && p.dates.is_empty() { return Err(bad("página de tarea sin estado ni fechas (estructura no reconocida)")); }
    let date_of = |keys: &[&str]| p.dates.iter().find_map(|l| {
        let low = l.to_lowercase();
        keys.iter().find(|k| low.starts_with(*k)).and_then(|_| l.split_once(':').and_then(|(_, rest)| pd(rest, TZ)))
    });
    let files = p.files.iter().map(|f| SubmittedFile { name: f.name.clone(), url: f.url.clone(), modified_at: f.time_text.as_deref().and_then(|t| pd(t, TZ)) }).collect();
    let raw = serde_json::json!({ "status": p.status_rows.iter().map(|r| [&r.label, &r.value]).collect::<Vec<_>>(), "grade": p.grade_rows.iter().map(|r| [&r.label, &r.value]).collect::<Vec<_>>(), "dates": p.dates });
    Ok(AssignmentInfo {
        cmid,
        opens_at: date_of(&["apertura", "abierto"]),
        due_at: date_of(&["cierre", "fecha de entrega", "entrega:", "vencimiento"]),
        submission_status: find(&p.status_rows, "estado de la entrega").map(Into::into),
        grading_status: find(&p.status_rows, "estado de la calificación").map(Into::into),
        time_remaining: find(&p.status_rows, "tiempo restante").map(Into::into),
        last_modified_at: find(&p.status_rows, "última modificación").and_then(|t| pd(t, TZ)),
        grade: p.grade_rows.iter().find(|r| r.label.trim().to_lowercase() == "calificación").map(|r| r.value.replace('\u{a0}', " ").trim().to_string()),
        feedback: find(&p.grade_rows, "retroalimentación").or_else(|| find(&p.grade_rows, "comentario")).map(Into::into),
        graded_at: find(&p.grade_rows, "calificado sobre").and_then(|t| pd(t, TZ)),
        grader: find(&p.grade_rows, "calificado por").map(Into::into),
        files, can_submit: p.can_add || p.can_edit, raw_rows: raw.to_string(),
        intro_text: p.intro_text.as_deref().and_then(|t| clean_text(t, 20_000)), intro_links: clean_links(&p.intro_links),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn intro_is_sanitized_and_bounded() {
        assert_eq!(clean_text("  Hola\u{0007}\nmundo  ", 100).as_deref(), Some("Hola\nmundo"));
        assert_eq!(clean_text("   ", 10), None);
        assert_eq!(clean_text(&"x".repeat(50), 10).unwrap().len(), 10);
        let l = |u: &str| IntroLink { name: "n".into(), url: u.into(), file: false };
        let out = clean_links(&[l("https://a/x"), l("javascript:alert(1)"), l("http://insegura"), l("https://a/x")]);
        assert_eq!(out.len(), 1, "solo https y sin duplicados");
    }
    #[test]
    fn rejects_unknown_selector_version_and_pages() {
        assert!(courses(json!({"selector_version":"v9","courses":[]})).is_err());
        assert!(courses(json!({"selector_version":"v1","more_pages":true,"courses":[]})).is_err());
    }
    #[test]
    fn rejects_duplicates_and_empty() {
        assert!(courses(json!({"selector_version":"v1","courses":[{"moodle_id":1,"name":"A"},{"moodle_id":1,"name":"B"}]})).is_err());
        assert!(contents(json!({"selector_version":"v1","course_id":1,"sections":[]})).is_err());
        assert!(contents(json!({"selector_version":"v1","course_id":null,"sections":[]})).is_err());
    }
    #[test]
    fn only_teacher_files_are_downloadable() {
        let b = "https://educacionadistancia.juntadeandalucia.es/centros/cordoba/pluginfile.php";
        assert!(downloadable(&format!("{b}/352905/mod_resource/content/3/a.pdf")));
        assert!(downloadable(&format!("{b}/1/mod_folder/content/0/x/y.zip")));
        assert!(downloadable(&format!("{b}/1/mod_assign/introattachment/0/enunciado.pdf")));
        assert!(!downloadable(&format!("{b}/1/assignsubmission_file/submission_files/9/mia.docx")), "mi entrega no es un recurso");
        assert!(!downloadable(&format!("{b}/1/user/icon/f1")));
        assert!(!downloadable("https://evil.example/centros/cordoba/pluginfile.php/1/mod_resource/content/3/a.pdf"));
    }
    fn assign_json() -> serde_json::Value {
        serde_json::json!({"selector_version":"v1","cmid":238692,"page_title":"x","dates":["Apertura: miércoles, 7 de octubre de 2026, 00:00","Cierre: viernes, 6 de noviembre de 2026, 23:59"],
          "status_rows":[{"label":"Estado de la entrega","value":"Enviado para calificar"},{"label":"Estado de la calificación","value":"Calificado"},{"label":"Tiempo restante","value":"La tarea fue enviada 9 días antes de la fecha límite"},{"label":"Última modificación","value":"viernes, 9 de octubre de 2026, 17:16"}],
          "grade_rows":[{"label":"Calificación","value":"9,00\u{a0}/\u{a0}100,00"},{"label":"Calificado sobre","value":"domingo, 11 de octubre de 2026, 20:33"},{"label":"Calificado por","value":"Profe, A"}],
          "files":[{"name":"Carta.pdf","url":"https://x/pluginfile.php/1/assignsubmission_file/submission_files/9/Carta.pdf","time_text":"9 de octubre de 2026, 17:16"}],"can_add":false,"can_edit":true})
    }
    #[test]
    fn assignment_maps_real_shaped_page() {
        let a = assignment(assign_json()).unwrap();
        assert_eq!(a.submission_status.as_deref(), Some("Enviado para calificar"));
        assert_eq!(a.grade.as_deref(), Some("9,00 / 100,00"));
        assert!(a.due_at.unwrap() > a.opens_at.unwrap());
        assert_eq!(a.files.len(), 1); assert!(a.files[0].modified_at.is_some() && a.can_submit);
        assert_eq!(a.last_modified_at, a.files[0].modified_at);
    }
    #[test]
    fn assignment_rejects_error_and_unknown_structure() {
        let mut e = assign_json(); e["error"] = "No puede acceder".into();
        assert!(assignment(e).is_err());
        assert!(assignment(serde_json::json!({"selector_version":"v1","cmid":5})).is_err());
        assert!(assignment(serde_json::json!({"selector_version":"v1","status_rows":[{"label":"a","value":"b"}]})).is_err(), "sin cmid");
    }
    #[test]
    fn dash_means_absent_not_a_value() {
        let mut j = assign_json(); j["status_rows"] = serde_json::json!([{"label":"Estado de la entrega","value":"Todavía no se han realizado envíos"},{"label":"Última modificación","value":"-"}]); j["files"] = serde_json::json!([]); j["grade_rows"] = serde_json::json!([]);
        let a = assignment(j).unwrap();
        assert_eq!((a.last_modified_at, a.grade, a.files.len()), (None, None, 0));
    }
    #[test]
    fn quiz_and_assign_date_labels() {
        let q = vec!["Abrió: lunes, 5 de octubre de 2026, 17:07".to_string(), "Cerró: lunes, 5 de octubre de 2026, 18:00".to_string()];
        let (o, c) = dates_from_lines(&q);
        assert_eq!(c.unwrap() - o.unwrap(), 53 * 60);
        let f = vec!["Se abrirá: martes, 10 de noviembre de 2026, 09:00".to_string()];
        assert!(dates_from_lines(&f).0.is_some() && dates_from_lines(&f).1.is_none());
        assert_eq!(dates_from_lines(&["Texto sin fecha".to_string()]), (None, None));
    }
    #[test]
    fn calendar_events_validate_and_extract_cmid() {
        let ok = calendar_events(json!({"selector_version":"v1","events":[
            {"id":35571,"title":"Vencimiento de X","component":"mod_assign","eventtype":"due","course_id":2278,"timestamp":1794175200,"url":"https://h/mod/assign/view.php?id=259543"},
            {"id":35571,"title":"dup","timestamp":1,"component":"","eventtype":""}]})).unwrap();
        assert_eq!(ok.len(), 1); assert_eq!(ok[0].cmid, Some(259543));
        assert!(calendar_events(json!({"selector_version":"v1","events":[{"id":1,"title":"sin fecha","component":"","eventtype":""}]})).is_err());
        assert!(calendar_events(json!({"selector_version":"v9","events":[]})).is_err());
    }
    #[test]
    fn parses_valid() {
        let (id, s) = contents(json!({"selector_version":"v1","course_id":7,"sections":[{"moodle_id":1,"position":0,"name":"General","activities":[{"cmid":5,"modname":"assign","name":"T","url":null,"description":null}]}]})).unwrap();
        assert_eq!((id, s[0].activities[0].cmid), (7, 5));
    }
}
