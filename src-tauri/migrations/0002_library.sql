ALTER TABLE course ADD COLUMN cover_path TEXT;           -- relativo a cache/covers
CREATE UNIQUE INDEX uq_resource_source ON course_resource(activity_id, source_url);
CREATE INDEX idx_local_file_name ON local_file(original_name);
