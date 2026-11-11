/** Secciones agrupadas por plataforma. El orden de `core` y `moodle` es el de siempre (Ctrl+1…9 no cambia). */
export type Group = 'core' | 'moodle' | 'familias';

export const groups: { id: Group; label: string | null }[] = [
  { id: 'core', label: null }, { id: 'moodle', label: 'Moodle' }, { id: 'familias', label: 'Familias' },
];

export const sections = [
  { id: 'dashboard', label: 'Dashboard', icon: 'dashboard', phase: 6, group: 'core' },
  { id: 'courses', label: 'Mis asignaturas', icon: 'courses', phase: 2, group: 'moodle' },
  { id: 'tasks', label: 'Tareas y actividades', icon: 'tasks', phase: 4, group: 'moodle' },
  { id: 'library', label: 'Biblioteca de archivos', icon: 'library', phase: 3, group: 'moodle' },
  { id: 'submissions', label: 'Entregas', icon: 'submissions', phase: 4, group: 'moodle' },
  { id: 'calendar', label: 'Calendario', icon: 'calendar', phase: 5, group: 'core' },
  { id: 'notifications', label: 'Notificaciones', icon: 'bell', phase: 5, group: 'core' },
  { id: 'moodle', label: 'Conexión', icon: 'plug', phase: 2, group: 'moodle' },
  { id: 'settings', label: 'Configuración', icon: 'settings', phase: 1, group: 'core' },
  // Familias: las vistas de datos solo aparecen cuando hay alumnado sincronizado; la conexión está siempre.
  { id: 'f_home', label: 'Resumen del alumno', icon: 'dashboard', phase: 6, group: 'familias', needsStudents: true },
  { id: 'f_grades', label: 'Calificaciones', icon: 'star', phase: 6, group: 'familias', needsStudents: true },
  { id: 'f_attendance', label: 'Faltas y retrasos', icon: 'clock', phase: 6, group: 'familias', needsStudents: true },
  { id: 'f_messages', label: 'Comunicaciones', icon: 'msg', phase: 6, group: 'familias', needsStudents: true },
  { id: 'f_conn', label: 'Conexión', icon: 'plug', phase: 6, group: 'familias' },
] as const;
export type SectionId = (typeof sections)[number]['id'];
/** Las diez primeras (las de antes de Familias) conservan sus atajos Ctrl+1…9 por orden. */
export const shortcutSections = sections.slice(0, 9);
