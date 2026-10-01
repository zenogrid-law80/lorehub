-- Keep backup ownership/catalog after a source repository is deleted.
CREATE TABLE repository_backups (
    id UUID PRIMARY KEY,
    resource_id TEXT NOT NULL,
    repository_name TEXT NOT NULL,
    storage_backend TEXT NOT NULL CHECK (storage_backend IN ('local_file','dynamodb_s3')),
    requested_by UUID NOT NULL REFERENCES users(id),
    status TEXT NOT NULL CHECK (status IN ('running','succeeded','failed')),
    stage TEXT NOT NULL DEFAULT 'exporting',
    manifest TEXT,
    archive_sha256 TEXT,
    size_bytes BIGINT,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX repository_backups_active ON repository_backups(resource_id) WHERE status='running';
CREATE INDEX repository_backups_repository ON repository_backups(resource_id,created_at DESC);

CREATE TABLE repository_restores (
    id UUID PRIMARY KEY,
    backup_id UUID NOT NULL REFERENCES repository_backups(id),
    requested_by UUID NOT NULL REFERENCES users(id),
    target_name TEXT NOT NULL,
    target_backend TEXT NOT NULL CHECK (target_backend IN ('local_file','dynamodb_s3')),
    target_resource_id TEXT,
    status TEXT NOT NULL CHECK (status IN ('running','succeeded','failed')),
    stage TEXT NOT NULL DEFAULT 'validating',
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX repository_restores_active_target ON repository_restores(lower(target_name)) WHERE status='running';

-- Quarantine persists after failures/restarts until a restore has been verified.
CREATE TABLE repository_restore_quarantine (
    resource_id TEXT PRIMARY KEY,
    restore_id UUID NOT NULL REFERENCES repository_restores(id)
);

-- CI resolves Sparse View names inside a repository; copies keep their original names.
DROP INDEX sparse_workspace_views_owner_name;
CREATE UNIQUE INDEX sparse_workspace_views_owner_repository_name
    ON sparse_workspace_views(owner_id,resource_id,lower(name));
