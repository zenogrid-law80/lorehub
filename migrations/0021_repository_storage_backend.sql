ALTER TABLE lore_resources
ADD COLUMN storage_backend TEXT NOT NULL DEFAULT 'dynamodb_s3'
CHECK (storage_backend IN ('dynamodb_s3', 'local_file'));
