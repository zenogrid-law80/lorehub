-- Shared Root revisions were indexed using the revision's original branch.
-- Rebuild derived snapshots and their cascading dependencies with the actual
-- Root branch identity. Preserve sync policies and operation history.
DELETE FROM repository_link_snapshots;
