ALTER TABLE reminder ADD COLUMN target_at INTEGER;      -- fecha para la que se calculó el aviso (si cambia, se rearma)
ALTER TABLE reminder ADD COLUMN title TEXT;
ALTER TABLE calendar_event ADD COLUMN remind_min INTEGER; -- aviso previo (minutos) en eventos personales
ALTER TABLE activity ADD COLUMN first_seen_at INTEGER;
UPDATE activity SET first_seen_at = coalesce(last_synced_at, strftime('%s','now'));
CREATE INDEX idx_notification_unread ON notification(created_at) WHERE read_at IS NULL;
CREATE INDEX idx_notification_unshown ON notification(created_at) WHERE shown_at IS NULL;
CREATE INDEX idx_event_activity ON calendar_event(activity_id);
