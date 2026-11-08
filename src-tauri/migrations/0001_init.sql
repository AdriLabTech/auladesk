-- AulaDesk esquema inicial. Fechas en UTC como epoch segundos (INTEGER).
-- Cada fila sincronizada conserva el id original de Moodle (moodle_id) y metadatos de sync.
PRAGMA foreign_keys = ON;

CREATE TABLE application_settings (
  key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at INTEGER NOT NULL
);

CREATE TABLE user_profile (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  moodle_user_id INTEGER, full_name TEXT, updated_at INTEGER NOT NULL
);

CREATE TABLE moodle_connection (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  base_url TEXT NOT NULL,
  adapter TEXT NOT NULL CHECK (adapter IN ('web','api')),
  session_state TEXT NOT NULL DEFAULT 'unknown' CHECK (session_state IN ('unknown','active','expired','logged_out')),
  moodle_release TEXT, last_checked_at INTEGER
);

CREATE TABLE course (
  id INTEGER PRIMARY KEY,
  moodle_id INTEGER NOT NULL UNIQUE,
  fullname TEXT NOT NULL, shortname TEXT, teacher TEXT, url TEXT,
  folder_name TEXT NOT NULL,          -- carpeta física saneada
  is_hidden INTEGER NOT NULL DEFAULT 0,
  removed_at INTEGER,                 -- desapareció de Moodle (se conserva)
  last_synced_at INTEGER
);

CREATE TABLE course_section (
  id INTEGER PRIMARY KEY,
  course_id INTEGER NOT NULL REFERENCES course(id) ON DELETE CASCADE,
  moodle_id INTEGER, position INTEGER NOT NULL, name TEXT NOT NULL, summary TEXT,
  removed_at INTEGER, last_synced_at INTEGER,
  UNIQUE (course_id, position)
);

-- Módulo de Moodle (cmid): recurso, tarea, cuestionario, foro...
CREATE TABLE activity (
  id INTEGER PRIMARY KEY,
  course_id INTEGER NOT NULL REFERENCES course(id) ON DELETE CASCADE,
  section_id INTEGER REFERENCES course_section(id) ON DELETE SET NULL,
  moodle_cmid INTEGER NOT NULL,
  modname TEXT NOT NULL,              -- assign, quiz, resource, folder, url, page...
  name TEXT NOT NULL, description TEXT, url TEXT,
  opens_at INTEGER, due_at INTEGER,
  moodle_status TEXT,                 -- estado leído de Moodle, sin interpretar
  local_status TEXT NOT NULL DEFAULT 'none',
  favorite INTEGER NOT NULL DEFAULT 0,
  done_local INTEGER NOT NULL DEFAULT 0,
  priority INTEGER NOT NULL DEFAULT 0,
  removed_at INTEGER, last_synced_at INTEGER, content_hash TEXT,
  UNIQUE (course_id, moodle_cmid)
);
CREATE INDEX idx_activity_due ON activity(due_at) WHERE due_at IS NOT NULL;
CREATE INDEX idx_activity_course ON activity(course_id, section_id);

CREATE TABLE course_resource (
  id INTEGER PRIMARY KEY,
  activity_id INTEGER NOT NULL REFERENCES activity(id) ON DELETE CASCADE,
  moodle_file_id TEXT, filename TEXT NOT NULL, mimetype TEXT, size_bytes INTEGER,
  source_url TEXT NOT NULL, modified_at INTEGER, removed_at INTEGER
);

CREATE TABLE assignment (
  activity_id INTEGER PRIMARY KEY REFERENCES activity(id) ON DELETE CASCADE,
  submission_status TEXT, grading_status TEXT, grade TEXT, feedback TEXT,
  max_file_size INTEGER, max_files INTEGER, accepted_types TEXT, last_synced_at INTEGER
);

CREATE TABLE assignment_attachment (
  id INTEGER PRIMARY KEY,
  activity_id INTEGER NOT NULL REFERENCES assignment(activity_id) ON DELETE CASCADE,
  resource_id INTEGER NOT NULL REFERENCES course_resource(id) ON DELETE CASCADE
);

-- Archivo en disco. Identificado por id interno; el hash evita duplicados.
CREATE TABLE local_file (
  id INTEGER PRIMARY KEY,
  uuid TEXT NOT NULL UNIQUE,
  original_name TEXT NOT NULL, local_name TEXT NOT NULL, rel_path TEXT NOT NULL UNIQUE,
  extension TEXT, size_bytes INTEGER NOT NULL, sha256 TEXT NOT NULL,
  source_url TEXT, moodle_ref TEXT,
  downloaded_at INTEGER NOT NULL, modified_at INTEGER,
  sync_state TEXT NOT NULL DEFAULT 'synced' CHECK (sync_state IN ('synced','outdated','missing','personal')),
  resource_id INTEGER REFERENCES course_resource(id) ON DELETE SET NULL
);
CREATE INDEX idx_local_file_hash ON local_file(sha256);

CREATE TABLE file_association (
  id INTEGER PRIMARY KEY,
  file_id INTEGER NOT NULL REFERENCES local_file(id) ON DELETE CASCADE,
  course_id INTEGER REFERENCES course(id) ON DELETE SET NULL,
  section_id INTEGER REFERENCES course_section(id) ON DELETE SET NULL,
  activity_id INTEGER REFERENCES activity(id) ON DELETE SET NULL,
  role TEXT NOT NULL CHECK (role IN ('resource','statement','work','submitted','personal')),
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_assoc_activity ON file_association(activity_id);

CREATE TABLE tag (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);
CREATE TABLE file_tag (file_id INTEGER REFERENCES local_file(id) ON DELETE CASCADE,
  tag_id INTEGER REFERENCES tag(id) ON DELETE CASCADE, PRIMARY KEY (file_id, tag_id));

-- Entregas: el historial NUNCA se sobrescribe ni se borra (sin ON DELETE CASCADE hacia aquí).
CREATE TABLE submission (
  id INTEGER PRIMARY KEY,
  activity_id INTEGER NOT NULL REFERENCES activity(id),
  deadline_at INTEGER, created_at INTEGER NOT NULL
);
CREATE TABLE submission_version (
  id INTEGER PRIMARY KEY,
  submission_id INTEGER NOT NULL REFERENCES submission(id),
  version INTEGER NOT NULL,
  file_id INTEGER NOT NULL REFERENCES local_file(id),   -- copia histórica
  sha256 TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('prepared','in_progress','sent','confirmed','pending_confirmation','rejected','error')),
  sent_at INTEGER, confirmed_at INTEGER,
  confirmation_evidence TEXT,          -- texto/estado leído de Moodle que prueba la recepción
  grade TEXT, teacher_feedback TEXT, error_detail TEXT,
  UNIQUE (submission_id, version)
);
CREATE TRIGGER submission_version_no_delete BEFORE DELETE ON submission_version
BEGIN SELECT RAISE(ABORT, 'el historial de entregas no se puede borrar'); END;
CREATE TRIGGER submission_version_file_immutable BEFORE UPDATE OF file_id, sha256, version ON submission_version
BEGIN SELECT RAISE(ABORT, 'archivo y hash de una versión son inmutables'); END;

CREATE TABLE calendar_event (
  id INTEGER PRIMARY KEY,
  source TEXT NOT NULL CHECK (source IN ('moodle','personal')),
  moodle_event_id INTEGER, activity_id INTEGER REFERENCES activity(id) ON DELETE SET NULL,
  kind TEXT NOT NULL,                  -- due, exam, quiz, course, study, reminder, goal...
  title TEXT NOT NULL, notes TEXT,
  starts_at INTEGER NOT NULL, ends_at INTEGER, all_day INTEGER NOT NULL DEFAULT 0,
  cancelled_at INTEGER, last_synced_at INTEGER,
  UNIQUE (source, moodle_event_id)
);
CREATE INDEX idx_event_start ON calendar_event(starts_at);

CREATE TABLE reminder (
  id INTEGER PRIMARY KEY,
  event_id INTEGER REFERENCES calendar_event(id) ON DELETE CASCADE,
  activity_id INTEGER REFERENCES activity(id) ON DELETE CASCADE,
  fire_at INTEGER NOT NULL, rule TEXT NOT NULL,   -- p.ej. 'due-7d','due-3d','due-1d','manual'
  fired_at INTEGER, cancelled_at INTEGER,
  UNIQUE (event_id, rule), UNIQUE (activity_id, rule)
);
CREATE INDEX idx_reminder_pending ON reminder(fire_at) WHERE fired_at IS NULL AND cancelled_at IS NULL;

CREATE TABLE notification (
  id INTEGER PRIMARY KEY,
  category TEXT NOT NULL, priority INTEGER NOT NULL DEFAULT 0,
  title TEXT NOT NULL, body TEXT, ref_kind TEXT, ref_id INTEGER,
  created_at INTEGER NOT NULL, read_at INTEGER, shown_at INTEGER
);

CREATE TABLE personal_note (
  id INTEGER PRIMARY KEY,
  course_id INTEGER REFERENCES course(id) ON DELETE CASCADE,
  activity_id INTEGER REFERENCES activity(id) ON DELETE CASCADE,
  body TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);

CREATE TABLE sync_job (
  id INTEGER PRIMARY KEY, kind TEXT NOT NULL, scope TEXT,
  state TEXT NOT NULL CHECK (state IN ('queued','running','done','failed','aborted')),
  started_at INTEGER, finished_at INTEGER
);
CREATE TABLE sync_history (
  id INTEGER PRIMARY KEY, job_id INTEGER REFERENCES sync_job(id) ON DELETE SET NULL,
  at INTEGER NOT NULL, level TEXT NOT NULL, message TEXT NOT NULL,
  selector_version TEXT, items_seen INTEGER, items_changed INTEGER
);
