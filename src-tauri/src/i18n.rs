//! Idioma de los avisos que genera el backend (notificaciones, resúmenes, avisos de Familias…).
//!
//! El texto nace en español y se traduce en UN punto, al guardar el aviso (`notif::add`), según el ajuste `ui_lang`
//! («auto» = idioma del sistema). Así la bandeja, las notificaciones de escritorio y el servicio en segundo plano salen en el mismo idioma.
//! Lo ya guardado no se retraduce al cambiar de idioma. Un texto sin plantilla conocida se deja tal cual (nunca se rompe).
use sqlx::SqlitePool;

/// (plantilla en español, plantilla en inglés). `{}` marca cada hueco, en el mismo orden en ambos idiomas.
const PAIRS: &[(&str, &str)] = &[
    ("Error de sincronización", "Sync error"),
    ("Documento descargado", "Document downloaded"),
    ("Entrega confirmada: {}", "Submission confirmed: {}"),
    ("Moodle muestra el archivo con el mismo SHA-256 que la copia preparada.", "Moodle shows the file with the same SHA-256 as the prepared copy."),
    ("Cambio de fecha: {}", "Date changed: {}"),
    ("Antes {} · ahora {}", "Before {} · now {}"),
    ("Nueva calificación: {}", "New grade: {}"),
    ("Nueva tarea: {}", "New task: {}"),
    ("Nuevo cuestionario: {}", "New quiz: {}"),
    ("Nuevo recurso: {}", "New resource: {}"),
    ("Entrega hoy: {}", "Due today: {}"),
    ("Entrega mañana: {}", "Due tomorrow: {}"),
    ("Entrega en {} días: {}", "Due in {} days: {}"),
    ("Examen hoy: {}", "Exam today: {}"),
    ("Examen mañana: {}", "Exam tomorrow: {}"),
    ("Examen en {} días: {}", "Exam in {} days: {}"),
    ("{} · cierra el {}", "{} · closes on {}"),
    ("El {}", "On {}"),
    ("A las {}", "At {}"),
    ("Resumen de hoy: {} entrega(s) hoy o mañana", "Today's summary: {} submission(s) due today or tomorrow"),
    ("Resumen semanal: {} entrega(s) en los próximos 7 días", "Weekly summary: {} submission(s) in the next 7 days"),
    ("Calificación modificada · {}: {}", "Grade changed · {}: {}"),
    ("Nueva calificación · {}: {}", "New grade · {}: {}"),
    ("Retraso el {}", "Lateness on {}"),
    ("Falta el {}", "Absence on {}"),
    ("Aviso del centro: {}", "School notice: {}"),
    ("Observación: {}", "Observation: {}"),
    ("Tutoría: {}", "Tutoring: {}"),
    ("Mensaje: {}", "Message: {}"),
    ("Cambio en la tutoría: {}", "Tutoring changed: {}"),
    ("Nueva tutoría: {}", "New tutoring: {}"),
    ("Notificación de prueba", "Test notification"),
];

/// Si `text` encaja con la plantilla (`{}` = cualquier trozo), devuelve los huecos.
fn capture<'a>(tpl: &str, text: &'a str) -> Option<Vec<&'a str>> {
    let parts: Vec<&str> = tpl.split("{}").collect();
    let mut rest = text.strip_prefix(parts[0])?;
    let mut out = Vec::new();
    for (i, lit) in parts.iter().enumerate().skip(1) {
        let last = i == parts.len() - 1;
        let at = if last { if lit.is_empty() { Some(rest.len()) } else if rest.ends_with(lit) { Some(rest.len() - lit.len()) } else { None } } else { rest.find(lit) }?;
        out.push(&rest[..at]);
        rest = &rest[at + lit.len()..];
    }
    rest.is_empty().then_some(out)
}

fn fill(tpl: &str, vals: &[&str]) -> String {
    let mut out = String::new();
    for (i, lit) in tpl.split("{}").enumerate() { if i > 0 { out.push_str(vals.get(i - 1).copied().unwrap_or("")); } out.push_str(lit); }
    out
}

/// Traduce un texto a inglés si coincide con alguna plantilla conocida; si no, lo devuelve igual. El texto puede ser multilínea (resúmenes).
pub fn to_en(text: &str) -> String {
    for (es, en) in PAIRS { if let Some(v) = capture(es, text) { return fill(en, &v); } }
    text.to_string()
}

/// ¿El idioma elegido (o, en «auto», el del sistema) es inglés?
pub async fn wants_english(pool: &SqlitePool) -> bool {
    match crate::db::get_setting(pool, "ui_lang").await.ok().flatten().as_deref() {
        Some("en") => true,
        Some("es") => false,
        #[cfg(test)]
        _ => false,
        #[cfg(not(test))]
        _ => std::env::var("LC_ALL").or_else(|_| std::env::var("LC_MESSAGES")).or_else(|_| std::env::var("LANG")).map(|l| l.to_lowercase().starts_with("en")).unwrap_or(false),
    }
}

/// Título y cuerpo en el idioma del usuario.
pub async fn localize(pool: &SqlitePool, title: &str, body: Option<&str>) -> (String, Option<String>) {
    if !wants_english(pool).await { return (title.to_string(), body.map(str::to_string)); }
    (to_en(title), body.map(to_en))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translates_known_templates_and_keeps_the_rest() {
        assert_eq!(to_en("Entrega mañana: Práctica 3"), "Due tomorrow: Practice 3".replace("Practice 3", "Práctica 3"));
        assert_eq!(to_en("Entrega en 3 días: Tarea: parte 2"), "Due in 3 days: Tarea: parte 2");
        assert_eq!(to_en("Antes 10/11/2026 10:00 · ahora 12/11/2026 10:00"), "Before 10/11/2026 10:00 · now 12/11/2026 10:00");
        assert_eq!(to_en("Interfaces · cierra el 12/11/2026 10:00"), "Interfaces · closes on 12/11/2026 10:00");
        assert_eq!(to_en("Nueva calificación · Matemáticas: 8"), "New grade · Matemáticas: 8");
        assert_eq!(to_en("Resumen semanal: 4 entrega(s) en los próximos 7 días"), "Weekly summary: 4 submission(s) in the next 7 days");
        assert_eq!(to_en("Texto que no conozco"), "Texto que no conozco");
    }
    #[test]
    fn every_pair_has_the_same_number_of_holes() {
        for (es, en) in PAIRS { assert_eq!(es.matches("{}").count(), en.matches("{}").count(), "{es}"); }
    }
    #[test]
    fn templates_roundtrip() {
        for (es, en) in PAIRS {
            let vals: Vec<String> = (0..es.matches("{}").count()).map(|i| format!("v{i}")).collect();
            let refs: Vec<&str> = vals.iter().map(String::as_str).collect();
            assert_eq!(to_en(&fill(es, &refs)), fill(en, &refs), "{es}");
        }
    }
}
