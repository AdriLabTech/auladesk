import { moodleAccount } from './api';

/** Cuenta de Moodle con la que se inició sesión (nombre leído de Moodle, nunca inventado). */
export const account = $state<{ name: string | null; userId: number | null }>({ name: null, userId: null });

export async function refreshAccount() {
  try { const a = await moodleAccount(); account.name = a.name; account.userId = a.user_id; } catch { /* sin BD aún */ }
}

/** Iniciales para el avatar: primera letra de las dos primeras palabras. */
export const initials = (name: string | null) => (name ? name.split(/[\s,]+/).filter(Boolean).slice(0, 2).map((w) => w[0]).join('').toUpperCase() : '?');
