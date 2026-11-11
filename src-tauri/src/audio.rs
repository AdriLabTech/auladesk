//! Ecualizador funcional: mide el sonido que sale por la salida de audio por defecto y emite el nivel de 16 bandas.
//!
//! Privacidad: el audio se captura con `parec` (monitor de la salida por defecto), se reduce aquí a 16 números de 0 a 255 y se descarta.
//! No se escribe en disco, no se envía a ninguna parte y solo corre mientras el efecto «Ecualizador» está activo y la ventana visible.
//! Solo Linux (PulseAudio/PipeWire). En otras plataformas la función devuelve un error y la interfaz usa la animación decorativa.
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use tauri::{AppHandle, Emitter};

pub const BANDS: usize = 16;
const RATE: usize = 24_000;
const N: usize = 1024;
const HOP: usize = 512;

/// FFT radix-2 in situ (N potencia de 2).
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 { j ^= bit; bit >>= 1; }
        j ^= bit;
        if i < j { re.swap(i, j); im.swap(i, j); }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f32::consts::PI / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let (tr, ti) = (re[b] * cr - im[b] * ci, re[b] * ci + im[b] * cr);
                re[b] = re[a] - tr; im[b] = im[a] - ti; re[a] += tr; im[a] += ti;
                let nr = cr * wr - ci * wi; ci = cr * wi + ci * wr; cr = nr;
            }
        }
        len <<= 1;
    }
}

/// Bandas de frecuencia espaciadas en escala logarítmica (40 Hz – 11 kHz).
fn band_edges() -> [usize; BANDS + 1] {
    let (lo, hi) = (40.0f32, 11_000.0f32);
    let mut e = [0usize; BANDS + 1];
    for (i, v) in e.iter_mut().enumerate() {
        let f = lo * (hi / lo).powf(i as f32 / BANDS as f32);
        *v = ((f / RATE as f32 * N as f32).round() as usize).clamp(1, N / 2);
    }
    for i in 1..=BANDS { if e[i] <= e[i - 1] { e[i] = (e[i - 1] + 1).min(N / 2); } }
    e
}

/// 1024 muestras (−1…1) → 16 niveles 0–255 (−75 dB … −12 dB del fondo de escala).
pub fn levels(samples: &[f32]) -> [u8; BANDS] {
    assert_eq!(samples.len(), N);
    let mut re: Vec<f32> = samples.iter().enumerate().map(|(i, s)| s * (0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (N - 1) as f32).cos())).collect();
    let mut im = vec![0.0f32; N];
    fft(&mut re, &mut im);
    let e = band_edges();
    let mut out = [0u8; BANDS];
    for b in 0..BANDS {
        let peak = (e[b]..e[b + 1].max(e[b] + 1).min(N / 2 + 1)).map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt()).fold(0.0f32, f32::max);
        let amp = peak / (N as f32 / 4.0); // seno a fondo de escala con ventana de Hann ≈ 1
        let db = 20.0 * amp.max(1e-9).log10();
        out[b] = (((db + 75.0) / 63.0).clamp(0.0, 1.0) * 255.0) as u8;
    }
    out
}

static RUN: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

#[tauri::command]
pub fn audio_viz_start(app: AppHandle) -> Result<(), String> {
    #[cfg(not(target_os = "linux"))]
    { let _ = app; return Err("El ecualizador con audio real solo está disponible en Linux.".into()); }
    #[cfg(target_os = "linux")]
    {
        use std::io::Read;
        use std::process::{Command, Stdio};
        let mut run = RUN.lock().unwrap();
        if run.is_some() { return Ok(()); }
        let mut child = Command::new("parec")
            .args(["-d", "@DEFAULT_MONITOR@", "--format=s16le", "--rate=24000", "--channels=1", "--latency-msec=30"])
            .stdout(Stdio::piped()).stderr(Stdio::null()).stdin(Stdio::null())
            .spawn().map_err(|e| format!("No se pudo ejecutar «parec» (paquete pulseaudio-utils / libpulse): {e}"))?;
        let mut out = child.stdout.take().ok_or("parec sin salida")?;
        let stop = Arc::new(AtomicBool::new(false));
        *run = Some(stop.clone());
        std::thread::spawn(move || {
            let mut win = vec![0.0f32; N];
            let mut buf = vec![0u8; HOP * 2];
            while !stop.load(Ordering::Relaxed) {
                if out.read_exact(&mut buf).is_err() { break; }
                win.rotate_left(HOP);
                for (i, c) in buf.chunks_exact(2).enumerate() { win[N - HOP + i] = i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0; }
                let _ = app.emit("audio-levels", levels(&win).to_vec());
            }
            let _ = child.kill(); let _ = child.wait();
            let still_ours = { let mut r = RUN.lock().unwrap(); if r.as_ref().is_some_and(|s| Arc::ptr_eq(s, &stop)) { *r = None; true } else { false } };
            if still_ours && !stop.load(Ordering::Relaxed) { let _ = app.emit("audio-viz-error", "La captura de audio se detuvo (¿sin servidor de sonido?)"); }
        });
        Ok(())
    }
}

#[tauri::command]
pub fn audio_viz_stop() {
    if let Some(s) = RUN.lock().unwrap().take() { s.store(true, Ordering::Relaxed); }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sine(f: f32, amp: f32) -> Vec<f32> { (0..N).map(|i| amp * (2.0 * std::f32::consts::PI * f * i as f32 / RATE as f32).sin()).collect() }
    fn peak_band(l: &[u8; BANDS]) -> usize { let m = *l.iter().max().unwrap(); l.iter().position(|v| *v == m).unwrap() }

    #[test]
    fn silence_is_zero() { assert_eq!(levels(&vec![0.0; N]), [0u8; BANDS]); }

    #[test]
    fn tone_lights_the_right_band_and_louder_means_higher() {
        let e = band_edges();
        for f in [100.0f32, 440.0, 1000.0, 4000.0, 9000.0] {
            let l = levels(&sine(f, 0.5));
            let k = (f / RATE as f32 * N as f32).round() as usize;
            let want = (0..BANDS).find(|b| k >= e[*b] && k < e[*b + 1]).unwrap();
            assert!((peak_band(&l) as i32 - want as i32).abs() <= 1, "{f} Hz → banda {} (esperada {want})", peak_band(&l));
        }
        let (q, loud) = (levels(&sine(1000.0, 0.02)), levels(&sine(1000.0, 0.6)));
        assert!(loud.iter().max() > q.iter().max(), "más volumen = más nivel");
        assert!(*loud.iter().max().unwrap() > 200, "un tono fuerte debe acercarse al máximo");
    }

    #[test]
    fn bands_are_increasing_and_cover_the_range() {
        let e = band_edges();
        assert!(e.windows(2).all(|w| w[1] > w[0]) && e[0] >= 1 && e[BANDS] <= N / 2);
    }
}
