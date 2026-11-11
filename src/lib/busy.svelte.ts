/** Trabajos largos (sincronizar, descargar). Mientras haya alguno, la interfaz se bloquea con un loader (SyncOverlay.svelte).
 *  Este módulo no importa nada de la app: api.ts lo usa para envolver las llamadas lentas sin ciclos. */
export interface Job { id: number; label: string }
export interface Progress { done: number; total: number; current: string | null }

export const busy = $state<{ jobs: Job[]; progress: Progress | null; blockSetting: boolean }>({ jobs: [], progress: null, blockSetting: true });
let seq = 0;

/** Ejecuta `fn` registrándola como trabajo en curso. Solo lo que lanza el usuario pasa por aquí: la sincronización automática corre en el backend y no bloquea. */
export async function withBusy<T>(label: string, fn: () => Promise<T>): Promise<T> {
  const id = ++seq;
  busy.jobs = [...busy.jobs, { id, label }];
  try { return await fn(); }
  finally {
    busy.jobs = busy.jobs.filter((j) => j.id !== id);
    if (!busy.jobs.length) busy.progress = null;
  }
}
export const isBusy = () => busy.jobs.length > 0;

/** Trabajos que lanza el backend por su cuenta (sincronización al iniciar): `names` = lo que falta; vacío = nada en curso. */
export function setExternal(names: string[]) {
  const rest = busy.jobs.filter((j) => j.id !== -1);
  busy.jobs = names.length ? [...rest, { id: -1, label: names.join(' · ') }] : rest;
  if (!busy.jobs.length) busy.progress = null;
}
