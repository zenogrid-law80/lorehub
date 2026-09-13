ALTER TABLE users ADD COLUMN display_name TEXT;

CREATE TABLE account_groups (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    description TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 500),
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX account_groups_owner_name ON account_groups(owner_id, lower(name));

CREATE TABLE account_group_members (
    group_id UUID NOT NULL REFERENCES account_groups(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY (group_id, user_id)
);
CREATE INDEX account_group_members_user ON account_group_members(user_id);

CREATE TABLE group_workspace_views (
    group_id UUID NOT NULL REFERENCES account_groups(id) ON DELETE CASCADE,
    resource_id TEXT NOT NULL REFERENCES lore_resources(resource_id) ON DELETE CASCADE,
    mode TEXT NOT NULL CHECK (mode IN ('full', 'sparse')),
    rules TEXT NOT NULL DEFAULT '' CHECK (octet_length(rules) <= 10000),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (group_id, resource_id)
);
