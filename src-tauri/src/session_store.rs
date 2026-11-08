//! Sesión persistente: las cookies de sesión del WebView se guardan en el llavero del sistema (Secret Service)
//! y se restauran en ese mismo WebView al arrancar. Nunca van a disco en claro ni a los logs, y no salen del navegador integrado.
//! Límite real: el servidor cierra la sesión tras 8 h de inactividad (M.cfg.sessiontimeout=28800); entonces hay que volver a entrar.
use serde::{Deserialize, Serialize};
use tauri::webview::cookie::{time::OffsetDateTime, Cookie, SameSite};

const SERVICE: &str = "moodledesk";
const USER: &str = "moodle-session-cookies";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredCookie {
    pub name: String, pub value: String, pub domain: Option<String>, pub path: Option<String>,
    pub secure: bool, pub http_only: bool, pub same_site: Option<String>, pub expires: Option<i64>,
}

/// Solo cookies del sitio de Moodle y de la pasarela de identidad de la Junta.
pub fn relevant(domain: Option<&str>) -> bool {
    domain.map(|d| d.trim_start_matches('.')).is_some_and(|d| d == "juntadeandalucia.es" || d.ends_with(".juntadeandalucia.es"))
}

pub fn to_stored(c: &Cookie<'_>) -> StoredCookie {
    StoredCookie {
        name: c.name().into(), value: c.value().into(), domain: c.domain().map(Into::into), path: c.path().map(Into::into),
        secure: c.secure().unwrap_or(false), http_only: c.http_only().unwrap_or(false),
        same_site: c.same_site().map(|s| match s { SameSite::Strict => "strict", SameSite::Lax => "lax", SameSite::None => "none" }.into()),
        expires: c.expires_datetime().map(|d| d.unix_timestamp()),
    }
}

pub fn to_cookie(s: &StoredCookie) -> Cookie<'static> {
    let mut b = Cookie::build((s.name.clone(), s.value.clone())).secure(s.secure).http_only(s.http_only);
    if let Some(d) = &s.domain { b = b.domain(d.clone()); }
    if let Some(p) = &s.path { b = b.path(p.clone()); }
    b = match s.same_site.as_deref() { Some("strict") => b.same_site(SameSite::Strict), Some("lax") => b.same_site(SameSite::Lax), Some("none") => b.same_site(SameSite::None), _ => b };
    if let Some(t) = s.expires.and_then(|t| OffsetDateTime::from_unix_timestamp(t).ok()) { b = b.expires(t); }
    b.build()
}

fn entry() -> Result<keyring::Entry, String> { keyring::Entry::new(SERVICE, USER).map_err(|e| e.to_string()) }

pub fn save(cookies: &[StoredCookie]) -> Result<(), String> {
    let json = serde_json::to_string(cookies).map_err(|e| e.to_string())?;
    entry()?.set_password(&json).map_err(|e| e.to_string())
}

pub fn load() -> Option<Vec<StoredCookie>> {
    let json = entry().ok()?.get_password().ok()?;
    serde_json::from_str::<Vec<StoredCookie>>(&json).ok().filter(|v| !v.is_empty())
}

pub fn forget() -> Result<(), String> {
    match entry()?.delete_credential() { Ok(()) | Err(keyring::Error::NoEntry) => Ok(()), Err(e) => Err(e.to_string()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_junta_domains_are_kept() {
        assert!(relevant(Some("educacionadistancia.juntadeandalucia.es")));
        assert!(relevant(Some(".edea.juntadeandalucia.es")));
        assert!(!relevant(Some("youtube.com")));
        assert!(!relevant(Some("evil-juntadeandalucia.es")));
        assert!(!relevant(None));
    }
    #[test]
    fn roundtrip_keeps_attributes() {
        let s = StoredCookie { name: "MoodleSessionX".into(), value: "v".into(), domain: Some("educacionadistancia.juntadeandalucia.es".into()), path: Some("/centros/cordoba/".into()),
            secure: true, http_only: true, same_site: Some("lax".into()), expires: None };
        assert_eq!(to_stored(&to_cookie(&s)), s);
        let p = StoredCookie { expires: Some(1900000000), ..s.clone() };
        assert_eq!(to_stored(&to_cookie(&p)).expires, Some(1900000000));
    }
}
