import { getSetting, setSetting } from './api';
import { themes, radii, readableOn, type RadiusId } from './themes';

/** Estado de apariencia: preset + acento propio opcional + redondeo + movimiento. Se persiste en los ajustes de la app. */
export const look = $state<{ id: string; accent: string | null; radius: RadiusId; calm: boolean; welcome: boolean }>({ id: 'brasa', accent: null, radius: 'normal', calm: false, welcome: true });

export const current = () => themes.find((t) => t.id === look.id) ?? themes[0];

/** Vuelca el estado en variables CSS del <html>. Lo no definido aquí lo deriva app.css. */
export function applyTheme() {
  const t = current(), s = document.documentElement.style, root = document.documentElement;
  const accent = look.accent ?? t.accent, fg = look.accent ? readableOn(look.accent) : t.accentFg;
  const set: Record<string, string> = { '--bg': t.bg, '--panel-solid': t.panel, '--text': t.text, '--muted': t.muted, '--accent': accent, '--accent-fg': fg, '--font-display': t.display ?? "'Space Grotesk', 'Inter Variable', system-ui, sans-serif" };
  for (const [k, v] of Object.entries(set)) s.setProperty(k, v);
  const [r, rl] = radii[look.radius]; s.setProperty('--radius', r); s.setProperty('--radius-lg', rl);
  root.dataset.mode = t.mode; root.dataset.theme = t.id; root.dataset.calm = look.calm ? '1' : '0';
}

export async function loadTheme() {
  try {
    const id = await getSetting('theme_id');
    if (id && themes.some((t) => t.id === id)) look.id = id;
    else look.id = (await getSetting('theme')) === 'light' ? 'hielo' : 'brasa'; // compatibilidad con el ajuste antiguo claro/oscuro
    const a = await getSetting('theme_accent'); look.accent = a && /^#[0-9a-f]{6}$/i.test(a) ? a : null;
    const r = (await getSetting('theme_radius')) as RadiusId | null; if (r && r in radii) look.radius = r;
    look.calm = (await getSetting('theme_calm')) === '1';
    look.welcome = (await getSetting('welcome_enabled')) !== '0';
  } catch (e) { console.error(e); }
  applyTheme();
}

const save = (k: string, v: string) => setSetting(k, v).catch(console.error);
export function setPreset(id: string) { look.id = id; look.accent = null; applyTheme(); save('theme_id', id); save('theme_accent', ''); }
export function setAccent(hex: string | null) { look.accent = hex; applyTheme(); save('theme_accent', hex ?? ''); }
export function setRadius(r: RadiusId) { look.radius = r; applyTheme(); save('theme_radius', r); }
export function setWelcome(w: boolean) { look.welcome = w; save('welcome_enabled', w ? '1' : '0'); }
export function setCalm(c: boolean) { look.calm = c; applyTheme(); save('theme_calm', c ? '1' : '0'); }
