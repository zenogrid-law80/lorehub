-- Sparse Views are selected by CI pipelines, not account groups.
-- Existing CI view definitions and pipeline snapshots remain intact.
DROP TABLE account_group_view_selections;
