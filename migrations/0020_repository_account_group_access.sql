CREATE TABLE repository_account_group_access (
    resource_id TEXT NOT NULL REFERENCES lore_resources(resource_id) ON DELETE CASCADE,
    group_id UUID NOT NULL REFERENCES account_groups(id) ON DELETE CASCADE,
    granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (resource_id, group_id)
);

CREATE INDEX repository_account_group_access_group
    ON repository_account_group_access(group_id, resource_id);
