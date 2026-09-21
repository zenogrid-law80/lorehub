CREATE TABLE repository_link_snapshots (
    root_resource_id TEXT NOT NULL REFERENCES lore_resources(resource_id) ON DELETE CASCADE,
    root_branch TEXT NOT NULL,
    root_revision TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (root_resource_id, root_branch)
);

CREATE TABLE repository_link_dependencies (
    root_resource_id TEXT NOT NULL REFERENCES lore_resources(resource_id) ON DELETE CASCADE,
    root_branch TEXT NOT NULL,
    root_revision TEXT NOT NULL,
    link_path TEXT NOT NULL CHECK (link_path <> '' AND octet_length(link_path) <= 4096),
    source_resource_id TEXT NOT NULL CHECK (source_resource_id <> '' AND octet_length(source_resource_id) <= 256),
    source_branch_id TEXT NOT NULL CHECK (source_branch_id <> '' AND octet_length(source_branch_id) <= 256),
    source_revision TEXT NOT NULL CHECK (source_revision ~ '^[0-9A-Fa-f]{64}$'),
    tracking BOOLEAN NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (root_resource_id, root_branch, link_path),
    FOREIGN KEY (root_resource_id, root_branch)
        REFERENCES repository_link_snapshots(root_resource_id, root_branch)
        ON DELETE CASCADE
);

CREATE INDEX repository_link_dependencies_source
    ON repository_link_dependencies (source_resource_id, source_branch_id);
