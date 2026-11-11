//! Recetas de navegación: qué pantallas del portal hay que recorrer y qué extractor ejecutar en cada una.
//! Son DATOS (JSON), no código: se validan antes de usarse y solo pueden hacer lo que permite esta lista cerrada de pasos.
//! Los extractores en sí viven en `selectors/familias.js` y solo devuelven datos al contrato de `mapper`.
use crate::error::{AppError, AppResult};
use serde::Deserialize;

pub const BUNDLED: &str = include_str!("../../selectors/familias_recipes.json");

pub const EXTRACTORS: [&str; 5] = ["students", "grades", "attendance", "messages", "events"];
pub const MESSAGE_KINDS: [&str; 4] = ["mensaje", "aviso", "observacion", "tutoria"];

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    /// Ruta dentro del portal (empieza por «/», sin «..», sin esquema ni host).
    Goto(String),
    /// Pulsa el primer enlace/botón visible cuyo texto contiene este.
    Click(String),
    /// Elige el alumno activo de la receta pulsando su nombre.
    ClickStudent(bool),
    /// Elige una opción (por su texto exacto) en un desplegable del portal, por su id.
    Select { id: String, option: String },
    WaitMs(u64),
    /// Ejecuta un extractor conocido; `kind` solo se usa con `messages`.
    Extract { extractor: String, kind: Option<String> },
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Recipe { pub name: String, #[serde(default)] pub per_student: bool, pub steps: Vec<Step> }

#[derive(Debug, Deserialize)]
struct File { version: String, recipes: Vec<Recipe> }

pub fn parse(json: &str) -> AppResult<Vec<Recipe>> {
    let f: File = serde_json::from_str(json).map_err(|e| AppError::Other(format!("recetas de Familias no válidas: {e}")))?;
    if f.version != super::mapper::SUPPORTED { return Err(AppError::Other(format!("recetas {} no soportadas", f.version))); }
    for r in &f.recipes { validate(r)?; }
    Ok(f.recipes)
}

fn validate(r: &Recipe) -> AppResult<()> {
    let bad = |m: &str| AppError::Other(format!("receta «{}»: {m}", r.name));
    if r.steps.is_empty() || r.steps.len() > 30 { return Err(bad("número de pasos no válido")); }
    let mut extracts = 0;
    for s in &r.steps {
        match s {
            Step::Goto(p) => {
                if !p.starts_with('/') || p.starts_with("//") || p.contains("..") || p.contains('\\') || p.contains("://") { return Err(bad("ruta no permitida")); }
            }
            Step::Click(t) => { if t.trim().is_empty() || t.chars().count() > 80 { return Err(bad("texto de pulsación no válido")); } }
            Step::ClickStudent(_) => { if !r.per_student { return Err(bad("click_student solo en recetas por alumno")); } }
            Step::Select { id, option } => { if id.is_empty() || id.len() > 40 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') || option.trim().is_empty() || option.chars().count() > 80 { return Err(bad("desplegable no válido")); } }
            Step::WaitMs(ms) => { if *ms > 15_000 { return Err(bad("espera demasiado larga")); } }
            Step::Extract { extractor, kind } => {
                extracts += 1;
                if !EXTRACTORS.contains(&extractor.as_str()) { return Err(bad("extractor desconocido")); }
                match (extractor.as_str(), kind) {
                    ("messages", Some(k)) if MESSAGE_KINDS.contains(&k.as_str()) => {}
                    ("messages", _) => return Err(bad("messages necesita un tipo válido")),
                    (_, Some(_)) => return Err(bad("solo messages admite tipo")),
                    _ => {}
                }
                if r.per_student && extractor == "students" { return Err(bad("la lista de alumnos no es por alumno")); }
            }
        }
    }
    if extracts == 0 { return Err(bad("sin ningún paso de extracción")); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_recipes_are_valid() {
        let r = parse(BUNDLED).unwrap();
        assert!(!r.is_empty());
        assert!(r.iter().all(|x| !x.per_student), "el portal solo muestra el perfil activo: no hay selector de alumno verificado");
    }
    #[test]
    fn select_step_is_validated() {
        let ok = r#"{"version":"f1","recipes":[{"name":"r","steps":[{"select":{"id":"X_CONVCENTRO","option":"1ª Evaluación"}},{"extract":{"extractor":"grades"}}]}]}"#;
        assert!(parse(ok).is_ok());
        assert!(parse(&ok.replace("X_CONVCENTRO", "a b;c")).is_err());
        assert!(parse(&ok.replace("1ª Evaluación", " ")).is_err());
    }

    #[test]
    fn valid_recipe_parses() {
        let r = parse(r#"{"version":"f1","recipes":[{"name":"notas","per_student":true,"steps":[{"goto":"/seneca/x"},{"click_student":true},{"click":"Calificaciones"},{"wait_ms":800},{"extract":{"extractor":"grades"}}]}]}"#).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].steps[4], Step::Extract { extractor: "grades".into(), kind: None });
    }
    #[test]
    fn dangerous_or_malformed_recipes_are_rejected() {
        let wrap = |step: &str, per: bool| format!(r#"{{"version":"f1","recipes":[{{"name":"r","per_student":{per},"steps":[{step},{{"extract":{{"extractor":"grades"}}}}]}}]}}"#);
        for bad in [r#"{"goto":"https://evil.example/"}"#, r#"{"goto":"//evil.example"}"#, r#"{"goto":"/a/../b"}"#, r#"{"goto":"relativa"}"#, r#"{"wait_ms":999999}"#, r#"{"click":" "}"#, r#"{"click_student":true}"#, r#"{"eval":"alert(1)"}"#] {
            assert!(parse(&wrap(bad, false)).is_err(), "{bad}");
        }
        assert!(parse(&wrap(r#"{"click_student":true}"#, true)).is_ok());
        assert!(parse(r#"{"version":"f9","recipes":[]}"#).is_err());
        assert!(parse(r#"{"version":"f1","recipes":[{"name":"r","steps":[{"goto":"/x"}]}]}"#).is_err(), "sin extracción");
        assert!(parse(r#"{"version":"f1","recipes":[{"name":"r","steps":[{"extract":{"extractor":"messages"}}]}]}"#).is_err(), "messages sin tipo");
        assert!(parse(r#"{"version":"f1","recipes":[{"name":"r","steps":[{"extract":{"extractor":"cualquiera"}}]}]}"#).is_err());
    }
}
