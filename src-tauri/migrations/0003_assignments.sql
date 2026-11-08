ALTER TABLE assignment ADD COLUMN last_modified_at INTEGER;
ALTER TABLE assignment ADD COLUMN time_remaining_text TEXT;
ALTER TABLE assignment ADD COLUMN graded_at INTEGER;
ALTER TABLE assignment ADD COLUMN grader TEXT;
ALTER TABLE assignment ADD COLUMN submitted_files_json TEXT;   -- [{name,url,modified_at}]
ALTER TABLE assignment ADD COLUMN can_submit INTEGER NOT NULL DEFAULT 0;
ALTER TABLE assignment ADD COLUMN raw_rows_json TEXT;          -- filas tal cual las muestra Moodle (por si cambia el idioma)
CREATE INDEX idx_submission_activity ON submission(activity_id);
