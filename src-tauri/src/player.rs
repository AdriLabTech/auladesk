//! Reproductor del ecualizador: muestra qué suena y deja pausar, cambiar de canción o saltar a un punto, en cualquier reproductor que
//! hable MPRIS (Spotify, VLC, navegadores…), mediante `playerctl`. Solo Linux. No hay dependencias nuevas ni acceso a la red:
//! no se descargan carátulas (serían peticiones a terceros) y nada de lo que suena se guarda ni se envía.
use serde::Serialize;
use std::process::{Command, Stdio};

const SEP: char = '\u{1f}';
const ACTIONS: [&str; 4] = ["play_pause", "next", "previous", "seek"];

#[derive(Serialize, Default, Debug, PartialEq, Clone)]
pub struct PlayerState {
    pub available: bool, pub reason: String, pub player: String,
    /// «Playing», «Paused» o «Stopped».
    pub status: String, pub title: String, pub artist: String, pub album: String,
    pub position: f64, pub length: f64,
}

fn playerctl(args: &[&str]) -> Result<String, String> {
    let out = Command::new("playerctl").args(args).stdin(Stdio::null()).stderr(Stdio::null()).output()
        .map_err(|_| "Falta «playerctl» (paquete playerctl) para controlar la música.".to_string())?;
    if out.status.success() { Ok(String::from_utf8_lossy(&out.stdout).trim_end_matches('\n').to_string()) } else { Err("sin reproductor".into()) }
}

/// Elige el reproductor: el que está sonando; si ninguno suena, el primero.
fn pick_player() -> Result<Option<String>, String> {
    let list = playerctl(&["-l"]).unwrap_or_default();
    let names: Vec<&str> = list.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if names.is_empty() { return if Command::new("playerctl").arg("--version").stdout(Stdio::null()).status().is_ok() { Ok(None) } else { Err("Falta «playerctl» (paquete playerctl) para controlar la música.".into()) }; }
    let playing = names.iter().find(|n| playerctl(&["-p", n, "status"]).is_ok_and(|s| s == "Playing"));
    Ok(Some(playing.unwrap_or(&names[0]).to_string()))
}

/// Interpreta la salida de `playerctl metadata --format …` (campos separados por U+001F; tiempos en microsegundos).
pub fn parse_state(line: &str) -> Option<PlayerState> {
    let f: Vec<&str> = line.split(SEP).collect();
    if f.len() != 7 { return None; }
    let us = |s: &str| s.trim().parse::<f64>().map(|v| (v / 1e6).max(0.0)).unwrap_or(0.0);
    Some(PlayerState { available: true, reason: String::new(), player: f[0].into(), status: f[1].into(), title: f[2].trim().into(), artist: f[3].trim().into(), album: f[4].trim().into(), length: us(f[5]), position: us(f[6]) })
}

pub fn read_state() -> PlayerState {
    let unavailable = |why: &str| PlayerState { reason: why.into(), ..Default::default() };
    let player = match pick_player() { Ok(Some(p)) => p, Ok(None) => return unavailable("No hay ningún reproductor abierto."), Err(e) => return unavailable(&e) };
    let fmt = format!("{{{{playerName}}}}{SEP}{{{{status}}}}{SEP}{{{{title}}}}{SEP}{{{{artist}}}}{SEP}{{{{album}}}}{SEP}{{{{mpris:length}}}}{SEP}{{{{position}}}}");
    match playerctl(&["-p", &player, "metadata", "--format", &fmt]).ok().as_deref().and_then(parse_state) {
        Some(s) => s,
        // Un reproductor abierto pero sin nada cargado: se puede dar a reproducir.
        None => PlayerState { available: true, player, status: playerctl(&["status"]).unwrap_or_else(|_| "Stopped".into()), ..Default::default() },
    }
}

#[tauri::command]
pub async fn player_state() -> PlayerState { tauri::async_runtime::spawn_blocking(read_state).await.unwrap_or_default() }

#[tauri::command]
pub async fn player_control(action: String, value: Option<f64>) -> Result<PlayerState, String> {
    if !ACTIONS.contains(&action.as_str()) { return Err("acción de reproductor no permitida".into()); }
    tauri::async_runtime::spawn_blocking(move || {
        let Some(p) = pick_player()? else { return Err("No hay ningún reproductor abierto.".to_string()) };
        let secs = value.filter(|v| v.is_finite() && *v >= 0.0 && *v < 86_400.0).map(|v| format!("{v:.1}"));
        match (action.as_str(), secs.as_deref()) {
            ("play_pause", _) => playerctl(&["-p", &p, "play-pause"])?,
            ("next", _) => playerctl(&["-p", &p, "next"])?,
            ("previous", _) => playerctl(&["-p", &p, "previous"])?,
            ("seek", Some(s)) => playerctl(&["-p", &p, "position", s])?,
            _ => return Err("falta el punto al que saltar".into()),
        };
        std::thread::sleep(std::time::Duration::from_millis(350)); // Spotify y otros tardan en reflejar el cambio; la interfaz se reconcilia después
        Ok(read_state())
    }).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_a_playing_track() {
        let l = format!("spotify{SEP}Playing{SEP}Canción: parte 1{SEP}Artista{SEP}Álbum{SEP}215000000{SEP}12500000");
        let s = parse_state(&l).unwrap();
        assert_eq!((s.player.as_str(), s.status.as_str(), s.title.as_str(), s.artist.as_str()), ("spotify", "Playing", "Canción: parte 1", "Artista"));
        assert_eq!((s.length, s.position), (215.0, 12.5));
    }
    #[test]
    fn tolerates_missing_numbers_and_rejects_garbage() {
        let s = parse_state(&format!("vlc{SEP}Paused{SEP}{SEP}{SEP}{SEP}{SEP}")).unwrap();
        assert_eq!((s.title.as_str(), s.length, s.position), ("", 0.0, 0.0));
        assert!(parse_state("sin separadores").is_none());
    }
    #[test]
    fn only_known_actions_are_allowed() { assert!(!ACTIONS.contains(&"volume") && ACTIONS.contains(&"seek")); }
}
