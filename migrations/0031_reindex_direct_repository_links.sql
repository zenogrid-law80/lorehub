-- Nested links belong to the repository behind their parent link. Rebuild
-- affected indexes so the root updater no longer attempts to change them.
DELETE FROM repository_link_snapshots snapshot
WHERE EXISTS (
    SELECT 1
    FROM repository_link_dependencies parent
    JOIN repository_link_dependencies child
      ON child.root_resource_id = parent.root_resource_id
     AND child.root_branch = parent.root_branch
     AND left(child.link_path, length(parent.link_path) + 1) = parent.link_path || '/'
    WHERE parent.root_resource_id = snapshot.root_resource_id
      AND parent.root_branch = snapshot.root_branch
);
