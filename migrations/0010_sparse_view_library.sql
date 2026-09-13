CREATE TABLE sparse_workspace_views (
    id UUID PRIMARY KEY,
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    resource_id TEXT NOT NULL REFERENCES lore_resources(resource_id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    mode TEXT NOT NULL CHECK (mode IN ('full', 'sparse')),
    rules TEXT NOT NULL DEFAULT '' CHECK (octet_length(rules) <= 10000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (id, resource_id)
);
CREATE UNIQUE INDEX sparse_workspace_views_owner_name
    ON sparse_workspace_views(owner_id, lower(name));
CREATE INDEX sparse_workspace_views_resource
    ON sparse_workspace_views(resource_id, lower(name));

CREATE TABLE account_group_view_selections (
    group_id UUID NOT NULL REFERENCES account_groups(id) ON DELETE CASCADE,
    resource_id TEXT NOT NULL REFERENCES lore_resources(resource_id) ON DELETE CASCADE,
    view_id UUID NOT NULL,
    selected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (group_id, resource_id),
    FOREIGN KEY (view_id, resource_id)
        REFERENCES sparse_workspace_views(id, resource_id) ON DELETE CASCADE
);
CREATE INDEX account_group_view_selections_view
    ON account_group_view_selections(view_id);

INSERT INTO sparse_workspace_views(id, owner_id, resource_id, name, mode, rules, updated_at)
SELECT md5(v.group_id::text || ':' || v.resource_id)::uuid,
       g.owner_id,
       v.resource_id,
       left(g.name, 40) || ' · ' || left(r.name, 35) || ' · ' || left(v.resource_id, 8),
       v.mode,
       v.rules,
       v.updated_at
FROM group_workspace_views v
JOIN account_groups g ON g.id = v.group_id
JOIN lore_resources r ON r.resource_id = v.resource_id;

INSERT INTO account_group_view_selections(group_id, resource_id, view_id, selected_at)
SELECT group_id,
       resource_id,
       md5(group_id::text || ':' || resource_id)::uuid,
       updated_at
FROM group_workspace_views;

DROP TABLE group_workspace_views;
