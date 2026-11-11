import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

/** Niveles (0–1) de 16 bandas del sonido que sale por la salida de audio por defecto. Los mide el backend (audio.rs) solo mientras
 *  hay un efecto activo y la ventana está visible; aquí solo se suavizan (sube al instante, baja poco a poco). */
export const BANDS = 16;
export const audio = $state<{ levels: number[]; live: boolean; error: string }>({ levels: Array(BANDS).fill(0), live: false, error: '' });
let un: UnlistenFn[] = [], wanted = false, busy: Promise<void> = Promise.resolve();

async function start() {
  try {
    un = [
      await listen<number[]>('audio-levels', (e) => { const L = audio.levels; for (let i = 0; i < BANDS; i++) { const v = (e.payload[i] ?? 0) / 255; L[i] = v > L[i] ? v : L[i] * 0.8 + v * 0.2; } }),
      await listen<string>('audio-viz-error', (e) => { audio.live = false; audio.error = e.payload; }),
    ];
    await invoke('audio_viz_start');
    audio.live = true; audio.error = '';
  } catch (e) { audio.live = false; audio.error = String(e); un.forEach((f) => f()); un = []; }
}
async function stop() {
  un.forEach((f) => f()); un = [];
  await invoke('audio_viz_stop').catch(() => {});
  audio.live = false; audio.levels = Array(BANDS).fill(0);
}
/** Pide (o deja de pedir) la captura. Las llamadas se encadenan para que arrancar y parar no se pisen. */
export function setAudioWanted(w: boolean) {
  if (w === wanted) return;
  wanted = w;
  busy = busy.then(() => (w ? start() : stop()));
}
