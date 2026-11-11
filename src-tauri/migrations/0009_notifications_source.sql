-- Origen de cada notificación y deduplicación entre plataformas (las existentes quedan sin origen = locales/Moodle).
ALTER TABLE notification ADD COLUMN source TEXT;
ALTER TABLE notification ADD COLUMN account_id INTEGER REFERENCES linked_account(id) ON DELETE CASCADE;
ALTER TABLE notification ADD COLUMN student_id INTEGER REFERENCES student(id) ON DELETE CASCADE;
ALTER TABLE notification ADD COLUMN dedupe_key TEXT;
CREATE UNIQUE INDEX uq_notification_dedupe ON notification(dedupe_key) WHERE dedupe_key IS NOT NULL;
CREATE INDEX idx_notification_source ON notification(source, account_id);
