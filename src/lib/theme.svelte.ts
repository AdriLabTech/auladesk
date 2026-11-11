import { getSetting, setSetting } from './api';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { themes, radii, readableOn, type RadiusId } from './themes';

/** Estado de apariencia: preset + acento propio opcional + redondeo + movimiento. Se persiste en los ajustes de la app. */
export const look = $state<{ id: string; accent: string | null; radius: RadiusId; calm: boolean; welcome: boolean; density: Density; scale: number; contrast: boolean; fx: Fx; welcomeKind: WelcomeKind; resolved: Exclude<WelcomeKind, 'random'> }>({ id: 'brasa', accent: null, radius: 'normal', calm: false, welcome: true, density: 'normal', scale: 1, contrast: false, fx: 'none', welcomeKind: 'reveal', resolved: 'reveal' });
export type WelcomeKind = 'reveal' | 'type' | 'terminal' | 'grid' | 'minimal' | 'random';
export const WELCOME_KINDS: WelcomeKind[] = ['reveal', 'type', 'terminal', 'grid', 'minimal', 'random'];
/** Elige el estilo concreto (si está en «aleatoria», sortea uno) y lo deja en `look.resolved`. */
export function resolveWelcome() {
  const real = WELCOME_KINDS.filter((k) => k !== 'random') as Exclude<WelcomeKind, 'random'>[];
  look.resolved = look.welcomeKind === 'random' ? real[Math.floor(Math.random() * real.length)] : look.welcomeKind;
}
export type Fx = 'none' | 'eq' | 'wave';
export type Density = 'compact' | 'normal' | 'roomy';
export const SCALES = [0.9, 1, 1.1, 1.25] as const;

/** ¿Hay que quitar el movimiento? Modo tranquilo o preferencia del sistema. */
export const still = () => look.calm || (typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches);

let cycleTimer: ReturnType<typeof setInterval> | undefined;
/** Temas animados: cada `secs` segundos se pasa a la siguiente parada del acento; la transición suave la hace CSS (@property --accent). */
function runCycle() {
  clearInterval(cycleTimer);
  const c = current().cycle;
  if (!c || look.accent || still()) return;
  let i = 0;
  cycleTimer = setInterval(() => { if (document.hidden) return; i = (i + 1) % c.stops.length; document.documentElement.style.setProperty('--accent', c.stops[i]); }, c.secs * 1000);
}

export const current = () => themes.find((t) => t.id === look.id) ?? themes[0];

/** Vuelca el estado en variables CSS del <html>. Lo no definido aquí lo deriva app.css. */
export function applyTheme() {
  const t = current(), s = document.documentElement.style, root = document.documentElement;
  const accent = look.accent ?? t.accent, fg = look.accent ? readableOn(look.accent) : t.accentFg;
  const set: Record<string, string> = { '--bg': t.bg, '--panel-solid': t.panel, '--text': t.text, '--muted': t.muted, '--accent': accent, '--accent-fg': fg, '--font-display': t.display ?? "'Space Grotesk', 'Inter Variable', system-ui, sans-serif" };
  for (const [k, v] of Object.entries(set)) s.setProperty(k, v);
  const [r, rl] = radii[look.radius]; s.setProperty('--radius', r); s.setProperty('--radius-lg', rl);
  root.dataset.mode = t.mode; root.dataset.theme = t.id; root.dataset.calm = look.calm ? '1' : '0'; root.dataset.density = look.density; root.dataset.contrast = look.contrast ? 'high' : 'normal';
  // Zoom nativo del webview de la app (los de Moodle/Familias no se tocan).
  runCycle();
  getCurrentWebview().setZoom(look.scale).catch(() => { /* sin permiso o sin ventana: se queda al 100 % */ });
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
    const d = await getSetting('ui_density'); if (d === 'compact' || d === 'roomy') look.density = d;
    const hc = await getSetting('ui_contrast'); look.contrast = hc === null ? matchMedia('(prefers-contrast: more)').matches : hc === '1';
    const ws = (await getSetting('welcome_style')) as WelcomeKind | null; if (ws && WELCOME_KINDS.includes(ws)) look.welcomeKind = ws; resolveWelcome();
    const fx = await getSetting('ui_fx'); if (fx === 'eq' || fx === 'wave') look.fx = fx;
    const z = Number(await getSetting('ui_scale')); if ((SCALES as readonly number[]).includes(z)) look.scale = z;
  } catch (e) { console.error(e); }
  applyTheme();
}

const save = (k: string, v: string) => setSetting(k, v).catch(console.error);
export function setPreset(id: string) { look.id = id; look.accent = null; applyTheme(); save('theme_id', id); save('theme_accent', ''); }
export function setAccent(hex: string | null) { look.accent = hex; applyTheme(); save('theme_accent', hex ?? ''); }
export function setRadius(r: RadiusId) { look.radius = r; applyTheme(); save('theme_radius', r); }
export function setWelcome(w: boolean) { look.welcome = w; save('welcome_enabled', w ? '1' : '0'); }
export function setCalm(c: boolean) { look.calm = c; applyTheme(); save('theme_calm', c ? '1' : '0'); }
export function setDensity(d: Density) { look.density = d; applyTheme(); save('ui_density', d); }
export function setScale(z: number) { look.scale = z; applyTheme(); save('ui_scale', String(z)); }
export function setContrast(c: boolean) { look.contrast = c; applyTheme(); save('ui_contrast', c ? '1' : '0'); }
export function setFx(f: Fx) { look.fx = f; save('ui_fx', f); }
export function setWelcomeKind(k: WelcomeKind) { look.welcomeKind = k; resolveWelcome(); save('welcome_style', k); }
