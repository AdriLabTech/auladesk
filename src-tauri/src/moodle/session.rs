//! Estado de sesión. La sesión real vive en las cookies del WebView; aquí solo se registra el estado observado.
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionState { Unknown, Active, Expired, LoggedOut }

/// Decide el estado a partir de la URL final tras cargar una página de Moodle.
/// Una respuesta 200 NO implica sesión: la página de login también devuelve 200.
pub fn classify_url(base: &str, final_url: &str) -> SessionState {
    if !final_url.starts_with(base) { return SessionState::Unknown; }
    let rest = &final_url[base.len()..];
    if rest.starts_with("login/") || rest.starts_with("/login/") { SessionState::LoggedOut } else { SessionState::Unknown }
}

/// Nombre de la cuenta leído de la página de Moodle. Es texto no fiable: se normaliza y se acota; si no parece un nombre se descarta.
pub fn clean_name(raw: &str) -> Option<String> {
    let s: String = raw.chars().filter(|c| !c.is_control()).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
    let n = s.chars().count();
    if !(2..=120).contains(&n) || s.contains('<') || s.contains('>') { return None; }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    const B: &str = "https://educacionadistancia.juntadeandalucia.es/centros/cordoba/";
    #[test]
    fn account_name_is_sanitized() {
        assert_eq!(clean_name("  Ana \n  López\t Ruiz "), Some("Ana López Ruiz".into()));
        assert_eq!(clean_name("A"), None);
        assert_eq!(clean_name(""), None);
        assert_eq!(clean_name("<script>x</script>"), None);
        assert_eq!(clean_name(&"x".repeat(121)), None);
    }
    #[test]
    fn login_redirect_is_not_a_session() {
        assert_eq!(classify_url(B, &format!("{B}login/index.php")), SessionState::LoggedOut);
        assert_eq!(classify_url(B, "https://otro.example/"), SessionState::Unknown);
    }
}
