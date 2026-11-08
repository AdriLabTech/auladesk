//! Fechas tal como las muestra Moodle en español («miércoles, 7 de octubre de 2026, 00:00»). Zona horaria explícita.
use chrono::{NaiveDate, NaiveTime, TimeZone};
use chrono_tz::Tz;

const MONTHS: [&str; 12] = ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];

/// Zona por defecto del Moodle de Córdoba (M.cfg dice «Europa/Madrid» en español; la IANA es Europe/Madrid).
pub const DEFAULT_TZ: Tz = chrono_tz::Europe::Madrid;

/// Devuelve segundos UTC. `None` si no reconoce el formato (nunca inventa una fecha).
pub fn parse_es_datetime(text: &str, tz: Tz) -> Option<i64> {
    let t = text.to_lowercase().replace('\u{a0}', " ");
    let words: Vec<&str> = t.split(|c: char| !(c.is_alphanumeric() || c == ':')).filter(|w| !w.is_empty()).collect();
    let mi = words.iter().position(|w| MONTHS.contains(w) || *w == "setiembre")?;
    let month = MONTHS.iter().position(|m| *m == words[mi]).map(|p| p as u32 + 1).unwrap_or(9);
    let day: u32 = words[..mi].iter().rev().find_map(|w| w.parse().ok().filter(|d| (1..=31).contains(d)))?;
    let year: i32 = words[mi + 1..].iter().find_map(|w| (w.len() == 4).then(|| w.parse().ok()).flatten())?;
    let time = words[mi + 1..].iter().find(|w| w.contains(':')).and_then(|w| {
        let (h, m) = w.split_once(':')?;
        NaiveTime::from_hms_opt(h.parse().ok()?, m.parse().ok()?, 0)
    }).unwrap_or_else(|| NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    let local = NaiveDate::from_ymd_opt(year, month, day)?.and_time(time);
    tz.from_local_datetime(&local).earliest().map(|d| d.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    fn utc(y: i32, m: u32, d: u32, h: u32, mi: u32) -> i64 { Utc.with_ymd_and_hms(y, m, d, h, mi, 0).unwrap().timestamp() }

    #[test]
    fn real_moodle_strings() {
        // 7 oct 2026 es horario de verano (CEST, UTC+2); 7 nov 2026 es invierno (CET, UTC+1).
        assert_eq!(parse_es_datetime("miércoles, 7 de octubre de 2026, 00:00", DEFAULT_TZ), Some(utc(2026, 10, 6, 22, 0)));
        assert_eq!(parse_es_datetime("viernes, 6 de noviembre de 2026, 23:59", DEFAULT_TZ), Some(utc(2026, 11, 6, 22, 59)));
        assert_eq!(parse_es_datetime("9 de octubre de 2026, 17:16", DEFAULT_TZ), Some(utc(2026, 10, 9, 15, 16)));
        assert_eq!(parse_es_datetime("lunes, 10 de noviembre de 2025, 23:59", DEFAULT_TZ), Some(utc(2025, 11, 10, 22, 59)));
    }
    #[test]
    fn dst_boundary_and_garbage() {
        // 25 oct 2026 02:30 ocurre dos veces; se toma la primera (CEST).
        assert_eq!(parse_es_datetime("domingo, 25 de octubre de 2026, 02:30", DEFAULT_TZ), Some(utc(2026, 10, 25, 0, 30)));
        assert_eq!(parse_es_datetime("-", DEFAULT_TZ), None);
        assert_eq!(parse_es_datetime("31 de febrero de 2026, 10:00", DEFAULT_TZ), None);
        assert_eq!(parse_es_datetime("", DEFAULT_TZ), None);
        assert_eq!(parse_es_datetime("12 de octubre", DEFAULT_TZ), None, "sin año no se inventa");
    }
    #[test]
    fn nbsp_is_handled() { assert!(parse_es_datetime("martes,\u{a0}13 de octubre de 2026,\u{a0}19:00", DEFAULT_TZ).is_some()); }
}
