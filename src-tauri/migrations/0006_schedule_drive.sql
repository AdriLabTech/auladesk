-- Entregas programadas: el usuario confirma una sola vez (archivo + SHA-256 + hora); a esa hora la app intenta entregar.
CREATE TABLE submission_schedule (
  id INTEGER PRIMARY KEY,
  version_id INTEGER NOT NULL REFERENCES submission_version(id),
  run_at INTEGER NOT NULL,
  state TEXT NOT NULL DEFAULT 'scheduled' CHECK (state IN ('scheduled','running','done','failed','cancelled','missed')),
  attempts INTEGER NOT NULL DEFAULT 0,
  last_attempt_at INTEGER, last_error TEXT,
  confirmed_sha256 TEXT NOT NULL,       -- hash que el usuario vio al confirmar; si la versión cambia, no se entrega
  replace_existing INTEGER NOT NULL DEFAULT 0,
  seen INTEGER NOT NULL DEFAULT 0,      -- el usuario ya vio el resultado (aviso emergente)
  created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX uq_schedule_active ON submission_schedule(version_id) WHERE state IN ('scheduled','running');
CREATE INDEX idx_schedule_state ON submission_schedule(state, run_at);

-- Google Drive: qué archivo (con qué contenido) ya está subido y las carpetas creadas por la app.
CREATE TABLE drive_item (
  id INTEGER PRIMARY KEY,
  local_file_id INTEGER NOT NULL REFERENCES local_file(id) ON DELETE CASCADE,
  sha256 TEXT NOT NULL,
  drive_file_id TEXT NOT NULL, drive_url TEXT,
  uploaded_at INTEGER NOT NULL,
  UNIQUE (local_file_id, sha256)
);
CREATE TABLE drive_folder (path TEXT PRIMARY KEY, drive_id TEXT NOT NULL);
