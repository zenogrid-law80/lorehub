ALTER TABLE pipelines ADD COLUMN trigger_patterns TEXT[] NOT NULL DEFAULT '{}';
ALTER TABLE pipelines ADD COLUMN changed_paths TEXT[] NOT NULL DEFAULT '{}';
ALTER TABLE pipelines ADD COLUMN changed_path_count INTEGER NOT NULL DEFAULT 0 CHECK (changed_path_count >= 0);
ALTER TABLE pipelines ADD COLUMN working_directory TEXT;
ALTER TABLE pipelines ADD COLUMN graph_definition TEXT;
