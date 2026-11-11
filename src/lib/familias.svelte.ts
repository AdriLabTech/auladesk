import { familiasStatus, familiasSetActiveStudent, type FamiliasStatus, type Student } from './api';

/** Estado compartido de Familias: la cuenta, el alumnado y el alumno ACTIVO. Cada vista pide los datos de ese alumno con su id explícito. */
export const fam = $state<{ status: FamiliasStatus | null; activeId: number | null }>({ status: null, activeId: null });

export async function refreshFamilias() {
  try {
    const s = await familiasStatus();
    fam.status = s;
    // El alumno activo es el guardado si sigue existiendo; si no, el primero; nunca uno de otra cuenta.
    const ids = s.students.map((x) => x.id);
    fam.activeId = s.active_student && ids.includes(s.active_student) ? s.active_student : (ids[0] ?? null);
  } catch { /* sin BD aún */ }
}

export async function pickStudent(id: number) {
  fam.activeId = id;
  try { await familiasSetActiveStudent(id); } catch (e) { console.error(e); }
}

export const activeStudent = (): Student | null => fam.status?.students.find((s) => s.id === fam.activeId) ?? null;
export const hasStudents = () => (fam.status?.students.length ?? 0) > 0;
