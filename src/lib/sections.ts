export const sections = [
  { id: 'dashboard', label: 'Dashboard', icon: 'dashboard', phase: 6 },
  { id: 'courses', label: 'Mis asignaturas', icon: 'courses', phase: 2 },
  { id: 'tasks', label: 'Tareas y actividades', icon: 'tasks', phase: 4 },
  { id: 'library', label: 'Biblioteca de archivos', icon: 'library', phase: 3 },
  { id: 'submissions', label: 'Entregas', icon: 'submissions', phase: 4 },
  { id: 'calendar', label: 'Calendario', icon: 'calendar', phase: 5 },
  { id: 'notifications', label: 'Notificaciones', icon: 'bell', phase: 5 },
  { id: 'moodle', label: 'Moodle / Conexión', icon: 'plug', phase: 2 },
  { id: 'settings', label: 'Configuración', icon: 'settings', phase: 1 },
] as const;
export type SectionId = (typeof sections)[number]['id'];
