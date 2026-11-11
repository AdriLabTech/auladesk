-- Cuentas vinculadas (una por plataforma) y alumnado de las cuentas familiares.
-- Los datos de Moodle NO cambian de tabla: se consideran de la cuenta Moodle (id 1, fija).
-- Nunca se guardan credenciales aquí: solo el nombre visible y el estado de la sesión.
CREATE TABLE linked_account (
  id INTEGER PRIMARY KEY,
  platform TEXT NOT NULL CHECK (platform IN ('moodle','familias','docentes')),
  label TEXT NOT NULL,
  external_user TEXT,
  state TEXT NOT NULL DEFAULT 'disconnected' CHECK (state IN ('connected','expired','disconnected','unavailable')),
  last_ok_at INTEGER, last_error TEXT,
  created_at INTEGER NOT NULL,
  UNIQUE (platform, external_user)
);
INSERT INTO linked_account(id, platform, label, external_user, state, created_at)
  VALUES (1, 'moodle', 'Moodle', (SELECT CAST(moodle_user_id AS TEXT) FROM user_profile WHERE id = 1), 'disconnected', strftime('%s','now'));

CREATE TABLE student (
  id INTEGER PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES linked_account(id) ON DELETE CASCADE,
  external_ref TEXT NOT NULL,            -- clave estable del portal (o hash del nombre si no hay id)
  display_name TEXT NOT NULL, center TEXT, course_group TEXT, tutor TEXT,
  first_seen_at INTEGER NOT NULL, last_synced_at INTEGER, removed_at INTEGER,
  UNIQUE (account_id, external_ref)
);

-- Historial de sincronizaciones por cuenta (el de Moodle sigue en sync_job / sync_history).
CREATE TABLE integration_sync_history (
  id INTEGER PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES linked_account(id) ON DELETE CASCADE,
  student_id INTEGER REFERENCES student(id) ON DELETE CASCADE,
  at INTEGER NOT NULL, ok INTEGER NOT NULL, kind TEXT NOT NULL,
  items_seen INTEGER NOT NULL DEFAULT 0, items_changed INTEGER NOT NULL DEFAULT 0,
  message TEXT
);
CREATE INDEX idx_isync_account ON integration_sync_history(account_id, at);
