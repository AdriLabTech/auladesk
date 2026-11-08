import { getSetting, setSetting } from './api';

/** Personalización del cartel de bienvenida del Dashboard. Textos con marcadores {nombre} {tareas} {vencidas}; se muestran siempre como TEXTO. */
export interface HeroCfg { title: string; subtitle: string; summary: boolean; date: boolean; account: boolean; ring: boolean; actions: boolean; tone: 'accent' | 'panel' }
export const HERO_DEFAULTS: HeroCfg = { title: '', subtitle: '', summary: true, date: true, account: true, ring: true, actions: true, tone: 'accent' };
export const hero = $state<HeroCfg>({ ...HERO_DEFAULTS });

export async function loadHero() {
  try {
    const raw = await getSetting('welcome_config'); if (!raw) return;
    const j = JSON.parse(raw) as Partial<HeroCfg>;
    Object.assign(hero, HERO_DEFAULTS, {
      title: typeof j.title === 'string' ? j.title.slice(0, 80) : '', subtitle: typeof j.subtitle === 'string' ? j.subtitle.slice(0, 200) : '',
      summary: j.summary !== false, date: j.date !== false, account: j.account !== false, ring: j.ring !== false, actions: j.actions !== false, tone: j.tone === 'panel' ? 'panel' : 'accent',
    });
  } catch { /* ajuste ausente o corrupto: valores por defecto */ }
}
export function saveHero() { setSetting('welcome_config', JSON.stringify($state.snapshot(hero))).catch(console.error); }
export function resetHero() { Object.assign(hero, HERO_DEFAULTS); saveHero(); }

export const fill = (t: string, v: { nombre: string | null; tareas: number; vencidas: number }) =>
  t.replace(/\{nombre\}/g, v.nombre ?? '').replace(/\{tareas\}/g, String(v.tareas)).replace(/\{vencidas\}/g, String(v.vencidas)).replace(/\s+([,.!¡¿?])/g, '$1').trim();
