import type { SectionId } from './sections';

/** Navegación compartida: la sección actual y un «destino» opcional dentro de ella (curso o tarea a abrir). */
export const nav = $state<{ section: SectionId; courseId: number | null; taskId: number | null; palette: boolean; welcome: boolean }>({
  section: 'dashboard', courseId: null, taskId: null, palette: false, welcome: false,
});

export function go(section: SectionId, target: { courseId?: number; taskId?: number } = {}) {
  nav.courseId = target.courseId ?? null;
  nav.taskId = target.taskId ?? null;
  nav.section = section;
}

/** Cada vista consume su destino una vez al montarse. */
export function takeTarget(kind: 'courseId' | 'taskId'): number | null {
  const v = nav[kind]; nav[kind] = null; return v;
}
