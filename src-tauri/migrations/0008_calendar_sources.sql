-- Reconstruye calendar_event: el CHECK de `source` solo admitía 'moodle' y 'personal' y SQLite no permite alterarlo.
-- sqlx ejecuta cada migración dentro de una transacción, donde PRAGMA foreign_keys no tiene efecto; por eso DROP TABLE
-- borraría en cascada los avisos (reminder) de estos eventos. Se copian a una tabla auxiliar y se restituyen tal cual
-- (mismos id y mismo estado disparado/cancelado). Se conservan TODOS los id de evento.
CREATE TABLE _reminder_bak AS SELECT * FROM reminder;
CREATE TABLE calendar_event_new (
  id INTEGER PRIMARY KEY,
  source TEXT NOT NULL CHECK (length(source) BETWEEN 1 AND 20),   -- moodle | personal | familias
  moodle_event_id INTEGER,                                          -- solo Moodle (id negativo = derivado de una actividad)
  activity_id INTEGER REFERENCES activity(id) ON DELETE SET NULL,
  kind TEXT NOT NULL,
  title TEXT NOT NULL, notes TEXT,
  starts_at INTEGER NOT NULL, ends_at INTEGER, all_day INTEGER NOT NULL DEFAULT 0,
  cancelled_at INTEGER, last_synced_at INTEGER, remind_min INTEGER,
  account_id INTEGER REFERENCES linked_account(id) ON DELETE CASCADE,   -- NULL en eventos personales
  student_id INTEGER REFERENCES student(id) ON DELETE CASCADE,          -- solo eventos de un alumno concreto
  external_id TEXT,                                                      -- id original en el portal de origen (no Moodle)
  UNIQUE (source, moodle_event_id)
);
INSERT INTO calendar_event_new(id, source, moodle_event_id, activity_id, kind, title, notes, starts_at, ends_at, all_day, cancelled_at, last_synced_at, remind_min, account_id)
  SELECT id, source, moodle_event_id, activity_id, kind, title, notes, starts_at, ends_at, all_day, cancelled_at, last_synced_at, remind_min,
         CASE WHEN source = 'moodle' THEN 1 END
  FROM calendar_event;
DROP TABLE calendar_event;
ALTER TABLE calendar_event_new RENAME TO calendar_event;
CREATE INDEX idx_event_start ON calendar_event(starts_at);
CREATE INDEX idx_event_activity ON calendar_event(activity_id);
CREATE INDEX idx_event_source ON calendar_event(source, account_id);
-- Sin duplicados al sincronizar: un id externo es único por origen, cuenta y alumno.
CREATE UNIQUE INDEX uq_event_external ON calendar_event(source, account_id, coalesce(student_id, 0), external_id) WHERE external_id IS NOT NULL;
INSERT OR IGNORE INTO reminder(id, event_id, activity_id, fire_at, rule, fired_at, cancelled_at, target_at, title)
  SELECT id, event_id, activity_id, fire_at, rule, fired_at, cancelled_at, target_at, title FROM _reminder_bak;
DROP TABLE _reminder_bak;
