import { getSetting, setSetting } from './api';
import { en } from './en';
import { errors } from './en_errors';

/** Idioma de la interfaz. Las cadenas fuente están en español: `t('texto')` devuelve la traducción si existe y, si no, el propio español
 *  (una cadena sin traducir nunca se rompe, solo se queda en español). Los mensajes que genera el backend siguen en español. */
export type LangPref = 'auto' | 'es' | 'en';
export const i18n = $state<{ pref: LangPref }>({ pref: 'auto' });

export const lang = (): 'es' | 'en' => (i18n.pref === 'auto' ? (typeof navigator !== 'undefined' && navigator.language?.toLowerCase().startsWith('en') ? 'en' : 'es') : i18n.pref);
export const locale = () => (lang() === 'en' ? 'en-GB' : 'es-ES');

export function t(s: string, vars?: Record<string, string | number>): string {
  let r = lang() === 'en' ? (en[s] ?? s) : s;
  if (vars) r = r.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? String(vars[k]) : m));
  return r;
}

export async function loadLang() {
  try { const v = await getSetting('ui_lang'); if (v === 'es' || v === 'en') i18n.pref = v; } catch (e) { console.error(e); }
  document.documentElement.lang = lang();
}
export function setLang(p: LangPref) { i18n.pref = p; document.documentElement.lang = lang(); setSetting('ui_lang', p).catch(console.error); }

/** Traduce un mensaje de error/estado del backend (en español). Acepta plantillas con `{}`; lo desconocido se devuelve igual. */
export function te(msg: unknown): string {
  const text = typeof msg === 'string' ? msg : msg instanceof Error ? msg.message : String(msg);
  if (lang() !== 'en') return text;
  for (const [es, en_] of errors) {
    const parts = es.split('{}');
    if (parts.length === 1) { if (text === es) return en_; continue; }
    if (!text.startsWith(parts[0])) continue;
    let rest = text.slice(parts[0].length); const vals: string[] = []; let ok = true;
    for (let i = 1; i < parts.length; i++) {
      const lit = parts[i], last = i === parts.length - 1;
      const at = last ? (lit === '' ? rest.length : rest.endsWith(lit) ? rest.length - lit.length : -1) : rest.indexOf(lit);
      if (at < 0) { ok = false; break; }
      vals.push(rest.slice(0, at)); rest = rest.slice(at + lit.length);
    }
    if (!ok || rest !== '') continue;
    let i = 0; return en_.replace(/\{\}/g, () => vals[i++] ?? '');
  }
  return text;
}
