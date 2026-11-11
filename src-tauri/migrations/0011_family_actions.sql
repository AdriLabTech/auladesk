-- Acciones de ESCRITURA en el portal de Familias (justificar una falta, pedir tutoría, enviar un mensaje).
-- Nada se envía sin que el usuario lo confirme explícitamente, y cada intento queda registrado (el registro no se borra).
CREATE TABLE family_action (
  id INTEGER PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES linked_account(id) ON DELETE CASCADE,
  student_id INTEGER NOT NULL REFERENCES student(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('justify_absence','request_tutoring','send_message')),
  target_id INTEGER,                         -- p. ej. attendance_record.id
  params_json TEXT NOT NULL,                 -- lo que vio y confirmó el usuario
  state TEXT NOT NULL DEFAULT 'prepared' CHECK (state IN ('prepared','running','done','failed','cancelled','unsupported')),
  created_at INTEGER NOT NULL, confirmed_at INTEGER, finished_at INTEGER, error TEXT
);
CREATE INDEX idx_family_action_student ON family_action(student_id, created_at);
