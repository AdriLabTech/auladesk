import { invoke as rawInvoke } from '@tauri-apps/api/core';
import { te } from './i18n.svelte';
/** Todas las llamadas al backend pasan por aquí: si fallan, el mensaje se muestra en el idioma elegido. */
const invoke = <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => rawInvoke<T>(cmd, args).catch((e) => { throw te(e); });
import { withBusy } from './busy.svelte';

export interface AppInfo { version: string; data_dir: string; db_path: string; session_type: string | null; schema_tables: number }

export const appInfo = () => invoke<AppInfo>('app_info');
export const getSetting = (key: string) => invoke<string | null>('get_setting', { key });
export const setSetting = (key: string, value: string) => invoke<void>('set_setting', { key, value });
export const backupDatabase = () => invoke<string>('backup_database');

export type SessionState = 'unknown' | 'active' | 'expired' | 'logged_out';
export const moodleOpen = () => invoke<void>('moodle_open');
export const moodleClose = () => invoke<void>('moodle_close');
export const moodleNavigate = (path: string) => invoke<void>('moodle_navigate', { path });
export const moodleProbe = (kind: 'dom' | 'session') => invoke<void>('moodle_probe', { kind });
export const moodleStatus = () => invoke<SessionState>('moodle_status');

export interface SyncSummary { courses: number; synced: number; assignments: number; failed: string[] }
export interface CourseRow { files: number; id: number; moodle_id: number; fullname: string; url: string | null; sections: number; activities: number; assignments: number; last_synced_at: number | null; removed_at: number | null }
export interface ActivityRow { files: number; id: number; section_position: number; section_name: string; modname: string; name: string; url: string | null }
export const moodleSync = () => withBusy('Moodle', () => invoke<SyncSummary>('moodle_sync'));
export const moodleSyncFull = () => withBusy('Moodle (completa)', () => invoke<SyncSummary>('moodle_sync_full'));
export const startupSyncStatus = () => invoke<string[]>('startup_sync_status');
export const listCourses = () => invoke<CourseRow[]>('list_courses');
export const courseTree = (courseId: number) => invoke<ActivityRow[]>('course_tree', { courseId });

export interface Stored { file_id: number; rel_path: string; original_name: string; duplicate: boolean; new_version: boolean; size: number }
export interface DownloadReport { stored: Stored[]; errors: string[] }
export interface CourseDownloadReport { activities: number; stored: number; duplicates: number; errors: string[] }
export interface FileRow {
  id: number; original_name: string; local_name: string; rel_path: string; extension: string | null; size_bytes: number; sha256: string;
  source_url: string | null; downloaded_at: number; sync_state: string; course_id: number | null; course_name: string | null;
  section_position: number | null; activity_id: number | null; activity_name: string | null; kind: string; exists: boolean;
}
export interface FileFilter { activity_id?: number | null; course_id?: number | null; kind?: string; q?: string; sort?: string; state?: string }
export const downloadActivity = (activityId: number) => withBusy('Descargando archivos', () => invoke<DownloadReport>('download_activity', { activityId }));
export const downloadCourse = (courseId: number, onlyMissing: boolean) => withBusy('Descargando curso', () => invoke<CourseDownloadReport>('download_course', { courseId, onlyMissing }));
export const courseCover = (courseId: number) => invoke<string | null>('course_cover', { courseId });
export const listFiles = (filter: FileFilter) => invoke<FileRow[]>('list_files', { filter });
export const listDuplicates = () => invoke<FileRow[][]>('list_duplicates');
export const listFolders = () => invoke<string[]>('list_folders');
export const fileRead = (id: number) => invoke<ArrayBuffer>('file_read', { id });
export const fileOpen = (id: number) => invoke<void>('file_open', { id });
export const fileReveal = (id: number) => invoke<void>('file_reveal', { id });
export const fileRename = (id: number, newName: string) => invoke<string>('file_rename', { id, newName });
export const fileMove = (id: number, destDir: string) => invoke<string>('file_move', { id, destDir });
export const folderCreate = (dir: string) => invoke<string>('folder_create', { dir });
export const fileDelete = (id: number) => invoke<void>('file_delete', { id });
export const importFile = (path: string, activityId?: number) => invoke<Stored>('import_file', { path, activityId });

export const moodleForgetSession = () => invoke<void>('moodle_forget_session');

export type TaskState = 'pendiente' | 'iniciada' | 'preparada' | 'entregada' | 'evaluada' | 'fuera_de_plazo' | 'sin_fecha' | 'sin_sincronizar';
export interface TaskRow {
  id: number; moodle_cmid: number; course_id: number; course_name: string; section_position: number | null; section_name: string | null; name: string; url: string | null;
  description: string | null; opens_at: number | null; due_at: number | null; priority: number; favorite: number; done_local: number; local_status: string;
  submission_status: string | null; grading_status: string | null; grade: string | null; feedback: string | null; time_remaining_text: string | null;
  last_modified_at: number | null; submitted_files_json: string | null; can_submit: number | null; assign_synced_at: number | null; last_synced_at: number | null;
  local_files: number; open_versions: number; latest_version_state: string | null; note: string | null; state: TaskState; seconds_to_due: number | null;
  intro_text: string | null; intro_links_json: string | null; raw_rows_json: string | null; grader: string | null; graded_at: number | null;
}
export interface VersionRow {
  id: number; submission_id: number; activity_id: number; activity_name: string; course_name: string; version: number; state: string;
  file_id: number; file_name: string; rel_path: string; sha256: string; size_bytes: number; created_at: number; sent_at: number | null; confirmed_at: number | null;
  confirmation_evidence: string | null; grade: string | null; teacher_feedback: string | null; error_detail: string | null; deadline_at: number | null;
}
export const taskList = () => invoke<TaskRow[]>('task_list');
export const taskSetFlags = (activityId: number, f: { priority?: number; favorite?: boolean; doneLocal?: boolean; localStatus?: '' | 'todo' | 'doing' }) => invoke<void>('task_set_flags', { activityId, ...f });
export const noteSet = (activityId: number, body: string) => invoke<void>('note_set', { activityId, body });
export const submissionPrepare = (activityId: number, path: string) => invoke<VersionRow>('submission_prepare', { activityId, path });
export const submissionHistory = (activityId?: number) => invoke<VersionRow[]>('submission_history', { activityId });
export const submissionCancel = (id: number) => invoke<void>('submission_cancel', { id });
export const submissionStart = (id: number, confirmed: boolean) => invoke<void>('submission_start', { id, confirmed });
export const submissionVerify = (id: number) => invoke<VersionRow>('submission_verify', { id });

export interface EventRow {
  id: number; source: 'moodle' | 'personal' | 'familias'; account_id: number | null; student_id: number | null; student_name: string | null; activity_id: number | null; kind: string; title: string; notes: string | null; starts_at: number; ends_at: number | null;
  all_day: number; cancelled_at: number | null; remind_min: number | null; course_name: string | null; activity_state: TaskState | null;
}
export interface NewEvent { title: string; kind: string; starts_at: number; ends_at?: number | null; all_day: boolean; notes?: string | null; remind_min?: number | null }
export const calendarList = (from: number, to: number) => invoke<EventRow[]>('calendar_list', { from, to });
export const calendarCreate = (event: NewEvent) => invoke<number>('calendar_create', { event });
export const calendarUpdate = (id: number, event: NewEvent) => invoke<void>('calendar_update', { id, event });
export const calendarDelete = (id: number) => invoke<void>('calendar_delete', { id });
export const calendarSetNotes = (id: number, notes: string) => invoke<void>('calendar_set_notes', { id, notes });

export interface NotifRow { id: number; category: string; priority: number; title: string; body: string | null; ref_kind: string | null; ref_id: number | null; created_at: number; read_at: number | null; shown_at: number | null; source: string | null; account_id: number | null; student_id: number | null }
export interface NotifSettings {
  lead_days: number[]; quiet_start: string; quiet_end: string; weekdays: boolean[]; muted: boolean;
  daily_digest: string | null; weekly_digest: string | null; weekly_day: number; categories: Record<string, boolean>;
}
export interface ServiceStatus { installed: boolean; active: boolean; unit_path: string }
export const notifList = (limit = 100, unreadOnly = false) => invoke<NotifRow[]>('notif_list', { limit, unreadOnly });
export const notifMarkRead = (id?: number) => invoke<void>('notif_mark_read', { id: id ?? null });
export const notifUnreadCount = () => invoke<number>('notif_unread_count');
export const notifSettingsGet = () => invoke<NotifSettings>('notif_settings_get');
export const notifSettingsSet = (settings: NotifSettings) => invoke<NotifSettings>('notif_settings_set', { settings });
export const notifTest = () => invoke<void>('notif_test');
export const serviceStatus = () => invoke<ServiceStatus>('service_status');
export const serviceInstall = () => invoke<ServiceStatus>('service_install');
export const serviceUninstall = () => invoke<ServiceStatus>('service_uninstall');

export interface PriorityItem { id: number; name: string; course: string; due_at: number | null; state: TaskState; priority: number; score: number; reason: string }
export interface DashCounts { courses: number; todo: number; overdue_recent: number; submitted: number; graded: number; undated: number; stale: number; exams_30d: number }
export interface CourseBar { course_id: number; name: string; done: number; todo: number }
export interface RecentItem { kind: string; title: string; detail: string | null; at: number }
export interface DashboardData { counts: DashCounts; priority: PriorityItem[]; events: EventRow[]; recent: RecentItem[]; per_course: CourseBar[]; last_sync_at: number | null; synced_tasks: number; total_tasks: number }
export const dashboardData = () => invoke<DashboardData>('dashboard_data');

export interface MoodleAccount { user_id: number | null; name: string | null }
export const moodleAccount = () => invoke<MoodleAccount>('moodle_account');

export interface AutoSyncStatus { enabled: boolean; minutes: number; last_at: number | null; last_ok: boolean | null; next_at: number | null; running: boolean; session_active: boolean; tray: boolean; close_to_tray: boolean }
export const autosyncStatus = () => invoke<AutoSyncStatus>('autosync_status');
export const submissionSubmit = (id: number, confirmed: boolean, replace = false) => invoke<VersionRow>('submission_submit', { id, confirmed, replace });

// ── Entregas programadas y aviso al abrir ──
export interface ScheduleRow {
  id: number; version_id: number; run_at: number; state: 'scheduled' | 'running' | 'done' | 'failed' | 'cancelled' | 'missed'; attempts: number;
  last_attempt_at: number | null; last_error: string | null; replace_existing: number; seen: number; created_at: number;
  activity_name: string; course_name: string; file_name: string; sha256: string; deadline_at: number | null;
}
export interface PendingItem { version: VersionRow; schedule: ScheduleRow | null; can_submit: number | null }
export interface PendingPrompt { items: PendingItem[]; results: ScheduleRow[] }
export const scheduleCreate = (versionId: number, runAt: number, confirmed: boolean, replace = false) => invoke<number>('schedule_create', { versionId, runAt, replace, confirmed });
export const scheduleList = (onlyActive = false) => invoke<ScheduleRow[]>('schedule_list', { onlyActive });
export const scheduleCancel = (id: number) => invoke<void>('schedule_cancel', { id });
export const pendingPromptData = () => invoke<PendingPrompt>('pending_prompt_data');
export const scheduleMarkSeen = () => invoke<void>('schedule_mark_seen');

// ── Copia en una carpeta sincronizada ──

export interface FolderStatus { path: string | null; available: boolean; submissions: boolean }
export interface CopyOutcome { file_id: number; name: string; status: 'copied' | 'skipped' | 'error'; detail: string | null }
export const folderBackupStatus = () => invoke<FolderStatus>('folder_backup_status');
export const folderBackupSet = (path: string | null) => invoke<void>('folder_backup_set', { path });
export const folderBackupSubmissionsSet = (on: boolean) => invoke<void>('folder_backup_submissions_set', { on });
export const folderBackupRun = (courseId?: number) => invoke<CopyOutcome[]>('folder_backup_run', { courseId });

// ── Plataformas (contrato común) ──
export type IntegrationState = { state: 'connected' | 'expired' | 'disconnected' } | { state: 'unavailable'; reason: string };
export interface IntegrationInfo { platform: 'moodle' | 'familias'; label: string; status: IntegrationState; last_ok_at: number | null; last_error: string | null }
export const integrationsList = () => invoke<IntegrationInfo[]>('integrations_list');
export const integrationSync = (platform: string) => invoke<string>('integration_sync', { platform });

// ── Familias (portal de familias y alumnado) ──
export interface Account { id: number; platform: string; label: string; external_user: string | null; state: string; last_ok_at: number | null; last_error: string | null }
export interface Student { id: number; account_id: number; external_ref: string; display_name: string; center: string | null; course_group: string | null; tutor: string | null; last_synced_at: number | null }
export interface FamiliasStatus { session: SessionState; account: Account; students: Student[]; active_student: number | null; paused: boolean; last_sync_at: number | null; last_error: string | null; recipes: number; window_open: boolean }
export interface FamiliasSummary { recipes: number; seen: number; changed: number; failed: string[] }
export interface GradeRow { id: number; subject: string; evaluation: string; grade_text: string; grade_num: number | null; observations: string | null; teacher: string | null; recorded_at: number | null; first_seen_at: number }
export interface AttendanceRow { id: number; day: string; slot: string | null; subject: string | null; kind: 'falta' | 'retraso' | 'otro'; justified: number; justification: string | null }
export interface MessageRow { id: number; kind: 'mensaje' | 'aviso' | 'observacion' | 'tutoria'; subject: string; sender: string | null; body: string | null; sent_at: number | null; read_in_portal: number; read_local_at: number | null; first_seen_at: number }
export interface Overview { grades: number; subjects: number; average: number | null; absences: number; delays: number; unjustified: number; unread: number; recent_grades: GradeRow[]; next_events: [string, number][] }
export interface ActionRow { id: number; student_id: number; kind: 'justify_absence' | 'request_tutoring' | 'send_message'; target_id: number | null; params_json: string; state: 'prepared' | 'running' | 'done' | 'failed' | 'cancelled' | 'unsupported'; created_at: number; confirmed_at: number | null; finished_at: number | null; error: string | null }
export interface ActionParams { attendance_id?: number; reason?: string; subject?: string; body?: string }
export const familiasOpen = () => invoke<void>('familias_open');
export const familiasClose = () => invoke<void>('familias_close');
export const familiasStatus = () => invoke<FamiliasStatus>('familias_status');
export const familiasSetActiveStudent = (studentId: number) => invoke<void>('familias_set_active_student', { studentId });
export const familiasSync = () => withBusy('Familias', () => invoke<FamiliasSummary>('familias_sync'));
export const familiasRecon = () => invoke<void>('familias_recon');
export const familiasPause = (paused: boolean) => invoke<void>('familias_pause', { paused });
export const familiasDisconnect = () => invoke<void>('familias_disconnect');
export const familiasGrades = (studentId: number) => invoke<GradeRow[]>('familias_grades', { studentId });
export const familiasAttendance = (studentId: number) => invoke<AttendanceRow[]>('familias_attendance', { studentId });
export const familiasMessages = (studentId: number | null) => invoke<MessageRow[]>('familias_messages', { studentId });
export const familiasMessageRead = (id: number) => invoke<void>('familias_message_read', { id });
export const familiasOverview = (studentId: number) => invoke<Overview>('familias_overview', { studentId });
export const familiasActionPrepare = (studentId: number, kind: ActionRow['kind'], params: ActionParams) => invoke<number>('familias_action_prepare', { studentId, kind, params });
export const familiasActionRun = (id: number, confirmed: boolean) => invoke<ActionRow>('familias_action_run', { id, confirmed });
export const familiasActionCancel = (id: number) => invoke<void>('familias_action_cancel', { id });
export const familiasActions = (studentId: number) => invoke<ActionRow[]>('familias_actions', { studentId });

export interface PlatformOutcome { platform: 'moodle' | 'familias'; label: string; ok: boolean; message: string; millis: number }
/** Sincroniza Moodle y Familias a la vez; cada una informa por separado. */
export const syncAll = () => withBusy('Moodle y Familias', () => invoke<PlatformOutcome[]>('sync_all'));
