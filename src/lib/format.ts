import { t, locale } from './i18n.svelte';
export const fmtSize = (b: number) => (b < 1024 ? `${b} B` : b < 1048576 ? `${(b / 1024).toFixed(0)} KB` : `${(b / 1048576).toFixed(1)} MB`);
export const fmtDate = (at: number | null) => (at ? new Date(at * 1000).toLocaleString(locale(), { dateStyle: 'medium', timeStyle: 'short' }) : t('nunca'));
export const modInfo: Record<string, { label: string; glyph: string; tone: string }> = {
  assign: { label: 'Tarea', glyph: 'edit', tone: 'amber' }, quiz: { label: 'Cuestionario', glyph: 'help', tone: 'violet' },
  resource: { label: 'Archivo', glyph: 'file', tone: 'blue' }, folder: { label: 'Carpeta', glyph: 'library', tone: 'blue' },
  url: { label: 'Enlace', glyph: 'external', tone: 'teal' }, page: { label: 'Página', glyph: 'text', tone: 'teal' },
  forum: { label: 'Foro', glyph: 'msg', tone: 'green' }, label: { label: 'Texto', glyph: 'text', tone: 'muted' },
  book: { label: 'Libro', glyph: 'courses', tone: 'teal' }, lti: { label: 'Herramienta', glyph: 'settings', tone: 'muted' }, imscp: { label: 'Paquete', glyph: 'box', tone: 'muted' },
};
export const info = (m: string) => { const i = modInfo[m] ?? { label: m, glyph: 'file', tone: 'muted' }; return { ...i, label: t(i.label) }; };
export const downloadable = (m: string) => m === 'resource' || m === 'folder' || m === 'assign';
/** Degradado estable por id para cursos sin portada. */
export const hue = (id: number) => (id * 47) % 360;

export const fmtDue = (at: number | null) => (at ? new Date(at * 1000).toLocaleString(locale(), { weekday: 'short', day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }) : 'Sin fecha límite');
/** «en 2 d 5 h» / «hace 6 d»: solo con datos reales; sin fecha no se inventa nada. */
export function fmtRemaining(seconds: number | null): string {
  if (seconds === null) return '';
  const abs = Math.abs(seconds), d = Math.floor(abs / 86400), h = Math.floor((abs % 86400) / 3600), m = Math.floor((abs % 3600) / 60);
  const txt = d > 0 ? `${d} d ${h} h` : h > 0 ? `${h} h ${m} min` : `${m} min`;
  return seconds >= 0 ? t('en {t}', { t: txt }) : t('hace {t}', { t: txt });
}
type Lab = Record<string, { label: string; tone: string }>;
/** Lee las etiquetas ya traducidas al idioma actual. */
const tr = (r: Lab): Lab => new Proxy(r, { get: (o, k: string) => (o[k] ? { ...o[k], label: t(o[k].label) } : o[k]) });
const _stateInfo: Lab = {
  pendiente: { label: 'Pendiente', tone: 'blue' }, iniciada: { label: 'Iniciada (borrador)', tone: 'violet' }, preparada: { label: 'Preparada', tone: 'teal' },
  entregada: { label: 'Entregada', tone: 'green' }, evaluada: { label: 'Evaluada', tone: 'green' }, fuera_de_plazo: { label: 'Fuera de plazo', tone: 'red' },
  sin_fecha: { label: 'Sin fecha', tone: 'muted' }, sin_sincronizar: { label: 'Sin sincronizar', tone: 'muted' },
};
const _versionState: Lab = {
  prepared: { label: 'Preparada', tone: 'teal' }, in_progress: { label: 'En proceso', tone: 'violet' }, sent: { label: 'Enviada', tone: 'blue' },
  confirmed: { label: 'Confirmada por Moodle', tone: 'green' }, pending_confirmation: { label: 'Pendiente de confirmación', tone: 'amber' },
  rejected: { label: 'Rechazada', tone: 'red' }, error: { label: 'Error / cancelada', tone: 'red' },
};

const _eventKinds: Lab = {
  due: { label: 'Entrega', tone: 'blue' }, exam: { label: 'Examen', tone: 'red' }, quiz: { label: 'Cuestionario', tone: 'violet' }, course: { label: 'Evento de curso', tone: 'teal' },
  reminder: { label: 'Recordatorio', tone: 'amber' }, study: { label: 'Sesión de estudio', tone: 'green' }, personal: { label: 'Fecha personal', tone: 'amber' },
  prep: { label: 'Preparación', tone: 'green' }, mock_exam: { label: 'Examen simulado', tone: 'red' }, goal: { label: 'Objetivo académico', tone: 'green' },
  tutoring: { label: 'Tutoría', tone: 'green' },
};
export const stateInfo = tr(_stateInfo), versionState = tr(_versionState), eventKinds = tr(_eventKinds);
export const personalKinds = ['reminder', 'study', 'personal', 'prep', 'mock_exam', 'goal'];
export const fmtTime = (at: number) => new Date(at * 1000).toLocaleTimeString(locale(), { hour: '2-digit', minute: '2-digit' });
export const fmtDay = (at: number) => new Date(at * 1000).toLocaleDateString(locale(), { weekday: 'long', day: 'numeric', month: 'long' });
export const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate());
export const addDays = (d: Date, n: number) => { const x = new Date(d); x.setDate(x.getDate() + n); return x; };
export const ts = (d: Date) => Math.floor(d.getTime() / 1000);
export const mondayOf = (d: Date) => addDays(startOfDay(d), -((d.getDay() + 6) % 7));
export const sameDay = (a: Date, b: Date) => a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();

export const greeting = () => { const h = new Date().getHours(); return t(h < 6 ? 'Buenas noches' : h < 14 ? 'Buenos días' : h < 21 ? 'Buenas tardes' : 'Buenas noches'); };
/** «hace 3 h» / «hace 2 d» a partir de un instante pasado. */
export function fmtAgo(at: number | null): string {
  if (!at) return t('nunca');
  const s = Math.max(0, Math.floor(Date.now() / 1000) - at);
  if (s < 90) return t('hace un momento');
  return t('hace {t}', { t: s < 3600 ? `${Math.floor(s / 60)} min` : s < 86400 ? `${Math.floor(s / 3600)} h` : `${Math.floor(s / 86400)} d` });
}
/** Etiqueta de día relativa: Hoy / Mañana / «jue 12 nov». */
export function dayLabel(at: number): string {
  const d = startOfDay(new Date(at * 1000)), n = Math.round((d.getTime() - startOfDay(new Date()).getTime()) / 86400000);
  return n === 0 ? t('Hoy') : n === 1 ? t('Mañana') : d.toLocaleDateString(locale(), { weekday: 'short', day: 'numeric', month: 'short' });
}
