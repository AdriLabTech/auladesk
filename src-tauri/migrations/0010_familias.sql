-- Datos del portal de familias. Todo lleva student_id: nunca se mezclan alumnos.
-- Mismo patrón que Moodle: id externo + hash del contenido + borrado lógico (lo que desaparece del portal se conserva marcado).
-- El detalle de las columnas se ajusta al DOM real tras la primera sesión de reconocimiento (ver INTEGRATION_RESEARCH.md).
CREATE TABLE grade_record (
  id INTEGER PRIMARY KEY,
  student_id INTEGER NOT NULL REFERENCES student(id) ON DELETE CASCADE,
  external_id TEXT NOT NULL,
  subject TEXT NOT NULL, evaluation TEXT NOT NULL,      -- p. ej. «1.ª Evaluación»
  grade_text TEXT NOT NULL, grade_num REAL,             -- texto tal cual (IN, SU, 7…) y valor si es numérico
  observations TEXT, teacher TEXT, recorded_at INTEGER,
  content_hash TEXT NOT NULL, first_seen_at INTEGER NOT NULL, last_synced_at INTEGER NOT NULL, removed_at INTEGER,
  UNIQUE (student_id, external_id)
);
CREATE TABLE attendance_record (
  id INTEGER PRIMARY KEY,
  student_id INTEGER NOT NULL REFERENCES student(id) ON DELETE CASCADE,
  external_id TEXT NOT NULL,
  day TEXT NOT NULL,                                    -- AAAA-MM-DD (hora local del centro)
  slot TEXT, subject TEXT,
  kind TEXT NOT NULL CHECK (kind IN ('falta','retraso','otro')),
  justified INTEGER NOT NULL DEFAULT 0, justification TEXT,
  content_hash TEXT NOT NULL, first_seen_at INTEGER NOT NULL, last_synced_at INTEGER NOT NULL, removed_at INTEGER,
  UNIQUE (student_id, external_id)
);
CREATE INDEX idx_attendance_day ON attendance_record(student_id, day);
CREATE TABLE school_message (
  id INTEGER PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES linked_account(id) ON DELETE CASCADE,
  student_id INTEGER REFERENCES student(id) ON DELETE CASCADE,     -- NULL = aviso para toda la cuenta
  external_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('mensaje','aviso','observacion','tutoria')),
  subject TEXT NOT NULL, sender TEXT, body TEXT, sent_at INTEGER,
  read_in_portal INTEGER NOT NULL DEFAULT 0, read_local_at INTEGER,
  content_hash TEXT NOT NULL, first_seen_at INTEGER NOT NULL, last_synced_at INTEGER NOT NULL, removed_at INTEGER
);
CREATE UNIQUE INDEX uq_message_external ON school_message(account_id, coalesce(student_id, 0), kind, external_id);
CREATE INDEX idx_message_sent ON school_message(student_id, sent_at);
