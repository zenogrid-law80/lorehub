# Database CI configuration

Migration `0036_database_ci_config.sql` adds repository/branch configuration heads,
immutable JSONB definition versions, change events and execution groups. Existing
branches and queued runs remain file-based until explicitly saved through CI settings.

## Editing and version history

**Save to database** validates the complete definition and creates a new version.
The active version and derived `ci_pipeline_routes` change in the same transaction.
Saving neither creates a Lore revision nor queues a pipeline. Code revisions and
configuration versions are displayed separately. Visual and TOML editors use the
same validation, including DAGs, paths, timeouts and concurrency limits.

The original TOML is retained for export and version diffs, including comments when
editing TOML directly. Visual edits regenerate TOML. Parsed JSONB is the execution
source of truth. Saving JSON is also supported through `configuration` instead of
`content`; only one may be supplied. Definitions remain limited to 256 KiB of TOML.

The client sends `expected_lock_version`. Concurrent saves return `409` and leave
the losing draft intact. Initial file imports also check `expected_revision` against
the branch head. Subsequent DB saves are independent of changes to the code head.
Restoring a version updates the active pointer, increments the lock version and
records an event; it never overwrites a historical definition.

New branches do not silently inherit another branch's DB settings. Select the new
branch, use **Copy from branch**, review the draft, then save. History remains stored
when a repository is deleted, but live repository access no longer exposes those
definitions or runs. Recreating a repository does not inherit its former settings.

## Execution and compatibility

At admission, each DB run records an immutable configuration version, code revision,
execution specification and resolved Sparse View rules. Later edits or preset
changes cannot affect queued or running work. The worker clones the recorded code
revision and uses the coordinator's specification, with no `.lore-ci.toml` required
in the checkout. The coordinator also creates job records from that specification.

Push admission and setting changes use the same repository lock. A push is deduplicated
by repository, branch and code revision; replaying it after an edit cannot select a
new configuration. Pipeline dependencies, queue diagnostics and related-run views
are confined to the same execution group. As before, only pipelines matching the
changed paths are queued, and absent prerequisites do not block execution. A manual
selection queues only the selected pipeline.

**Rerun original group** duplicates every member of the original group, including
commands and resolved Sparse View rules, with a new group ID. It retains the old
code revision even if the branch has advanced. To use current settings, start a new
pipeline from CI settings instead. Legacy file runs continue using their original
execution path and can be started again from CI settings.

New workers advertise `X-LoreHub-Execution-Spec-Version: 1` on coordinator requests.
Both keyed and legacy claim routes exclude DB runs without that capability. Existing
file runs remain eligible for old workers. Update the coordinator and workers before
converting branches; DB runs wait if no capable worker is available. Roll out one
branch first, verify manual and push execution, then import additional branches.

To return a branch to file mode, export the active TOML, commit and push it to that
branch, then choose **Use committed TOML**. The coordinator checks that its parsed
definition matches the active DB version before switching. Already queued DB runs
retain their original execution specifications and still need a capable worker.
Do not downgrade the coordinator while such runs are queued or running. Include
PostgreSQL in backups: repository files alone no longer contain DB-mode CI settings.

## HTTP API

All configuration endpoints require the same live repository access as CI settings;
mutations also require CSRF validation. Incoming Lore link sources remain ineligible
for CI configuration changes. Runner APIs use runner tokens and active leases.

| Endpoint | Behavior |
| --- | --- |
| `GET /api/v1/repositories/{name}/ci-config?branch=main` | Current content, model, code revision, source mode, lock and configuration versions |
| `POST /api/v1/repositories/{name}/ci-config` | Save `branch`, `expected_revision`, `expected_lock_version` and `content` or `configuration` |
| `GET .../ci-config/history?branch=main` | Latest 100 versions and 100 change events |
| `GET .../ci-config/versions/{id}?branch=main` | Scoped immutable version, including TOML and parsed definition |
| `POST .../ci-config/restore` | Activate `revision_id` using `branch` and `expected_lock_version` |
| `POST .../ci-config/file-mode` | Verify committed TOML and switch using `branch` and `expected_lock_version` |
| `GET .../pipelines?branch=main&revision=HASH` | Pipeline choices carrying their configuration version ID |
| `POST /api/v1/pipelines` | Existing fields plus optional `config_revision_id`; DB selection requires a branch |
| `GET /api/v1/pipelines/{id}` | Includes the frozen `execution_spec` for authorized viewers |
| `POST /api/v1/pipelines/{id}/rerun` | Queue a new group using the original specifications |

New run submission still checks the selected code revision against the current branch
head. Explicit configuration IDs must belong to the selected repository and branch.
Omitting a configuration ID chooses the active version at admission. Historical reruns
use the dedicated rerun endpoint and do not require the original code to remain the head.

## Visual graph positions

Migration `0037_ci_graph_layouts.sql` stores shared node positions in PostgreSQL,
separately for each repository, branch and graph (overview or a specific pipeline).
Drag pipeline cards, stage headers or job cards to move them; Alt + arrow keys move
10 pixels, or 40 with Shift. Moving a stage also moves its jobs. Positions save
automatically and are restored when reopening the graph or refreshing the page.
The toolbar shows pending or failed saves and offers retry and **Reset layout**.
Reset affects only the current graph and restores automatic positioning.

Layouts work in both file and DB configuration modes. Moving a node never changes
execution order, TOML, configuration versions or queued runs. Repository members
share the layout; simultaneous changes to different nodes are merged, and the last
accepted change to the same node wins. Other open sessions see updates when they
reload or reopen the graph. Failed saves remain in the current browser session for
retry; leaving the page while changes remain unsaved triggers a warning.

Node identity uses pipeline, stage and job names rather than list indexes. Reordering
definitions preserves positions; renamed or new nodes start at automatic positions.
Old name entries remain until reset. Deleting the repository removes its layouts.

`GET .../ci-config/layout?branch=main&graph=...` returns `{ "positions": {...} }`.
The `graph` query value is URL-encoded JSON: `["overview"]`,
`["detail","pipeline-name"]`, or `["detail",null]` for the legacy manual pipeline.
`POST .../ci-config/layout` accepts `branch`, `graph` and a `positions` patch with
node keys mapped to `{ "x": 100, "y": 200 }` (or `null` to remove one position).
Alternatively, send `reset: true` without positions. Coordinates are in unscaled
graph pixels; job coordinates are relative to their stage. The same repository
authorization and CSRF checks apply. Layouts are limited to 2,000 positions and
256 KiB, with coordinates between 0 and 50,000.
