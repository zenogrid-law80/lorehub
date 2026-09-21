-- Policies survive snapshot re-indexing; Lore remains the link definition source.
CREATE TABLE repository_link_policies (
    root_resource_id TEXT NOT NULL REFERENCES lore_resources(resource_id) ON DELETE CASCADE,
    root_branch TEXT NOT NULL,
    link_path TEXT NOT NULL,
    auto_update BOOLEAN NOT NULL DEFAULT true,
    last_success_at TIMESTAMPTZ,
    last_error TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (root_resource_id, root_branch, link_path)
);

CREATE TABLE repository_link_operations (
    id UUID PRIMARY KEY,
    root_resource_id TEXT NOT NULL REFERENCES lore_resources(resource_id) ON DELETE CASCADE,
    root_branch TEXT NOT NULL,
    requested_by UUID REFERENCES users(id) ON DELETE SET NULL,
    request TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'running' CHECK (status IN ('running','succeeded','failed','partial')),
    stage TEXT NOT NULL DEFAULT 'validating',
    source_ready BOOLEAN NOT NULL DEFAULT false,
    source_path_created BOOLEAN NOT NULL DEFAULT false,
    result_revision TEXT,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX repository_link_operations_root ON repository_link_operations(root_resource_id,root_branch,created_at DESC);
