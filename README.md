# LoreHub

LoreHub is a Lore VCS development platform that brings CI execution into a coordinator and worker service. The web dashboard manages Lore repositories, starts pipelines for a specific revision, and shows status, jobs, logs, and cancellation controls. Pipeline settings are read from the `.lore-ci.toml` included in the requested revision.

LoreHub does not use Git checkouts, GitLab APIs, or Git commit SHAs. It uses Lore's 64-character revision hashes.

```text
User ── Google login (`@zenogrid.co.kr`)
      │ session cookie + CSRF token
      │ POST /api/v1/pipelines
      ▼
 API coordinator ───── PostgreSQL
      ▲
      │ HTTPS: claim / heartbeat / status / logs
 Rust workers × N
                           │
                 lore clone --revision HASH
                           │
                    .lore-ci.toml
                           │
                     /bin/sh -e
```

## Project layout

```text
src/main.rs             CLI arguments and process signals
src/server/             HTTP server, Google OIDC, and APIs
src/ci/                 pipeline configuration, queue, state, and logs
src/runner/             workers and shell executor
src/vcs/lore.rs         Lore CLI command construction
migrations/             PostgreSQL schema
examples/               .lore-ci.toml examples
web/                    embedded dashboard HTML, CSS, and JavaScript
```

The binary and Rust crate are both named `lorehub`. `lorehub serve` starts the coordinator and `lorehub worker` starts a worker. Lore arguments are built by `vcs`; child environment variables, timeouts, and process termination are handled by `runner`.

The dashboard uses deferred scripts without a JavaScript build step. `web/app.js` owns shared state, navigation setup, and polling; `web/execution-detail.js` owns the execution drawer, summary, cancellation, and bounded log paging. Execution graphs and analysis remain in `web/execution-graph.js` and `web/execution-analysis.js`. Load all deferred scripts before the `DOMContentLoaded` initialization in `app.js`.

## Migrating from lore-runner

| Previous | LoreHub |
| --- | --- |
| `lore-runner` binary / `lore_runner` crate | `lorehub` |
| Static API bearer token | Google Workspace login session |
| `LORE_RUNNER_BIND` | `LOREHUB_BIND` |
| `LORE_RUNNER_WORK_DIR` | `LOREHUB_WORK_DIR` |
| `lore-runner-api.service` | `lorehub-api.service` |
| `lore-runner-worker@.service` | `lorehub-worker@.service` |
| `/etc/lore-runner/environment` | `/etc/lorehub/environment` |
| `RUST_LOG=lore_runner=info` | `RUST_LOG=lorehub=info` |

The coordinator requires `DATABASE_URL`, `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, and `LOREHUB_PUBLIC_URL`. Workers do not require `DATABASE_URL`. Existing `LORE_BIN`, `/api/v1`, `.lore-ci.toml`, and `LORE_*` job variables remain valid. Legacy static tokens and `LORE_RUNNER_*` settings are not read. Migration `0002_google_auth.sql` adds authentication tables without changing pipeline, job, or log data.

## Features

- Google Workspace OIDC restricted to `@zenogrid.co.kr`.
- Responsive dashboard with repository, branch, pipeline, status, and text filters.
- Expandable repository trees with `LINK` badges and revision-pinned reads.
- Repository links with pinned revisions, automatic/manual synchronization, and retryable operations.
- Visual/TOML CI editing, dependency graphs, change-path preview, validation, undo, redo, and diffs.
- Pipeline details with execution graphs, status, jobs, and paged logs.
- System, light, and dark themes.
- Runner inventory, Docker status, maintenance mode, bounded retries, heartbeats, leases, and self-update verification.
- PostgreSQL sessions, HttpOnly/SameSite cookies, CSRF protection, atomic job claiming, and embedded migrations.

The shell executor runs POSIX shell on Linux and PowerShell on Windows. Jobs have the worker account's file and network permissions; use a dedicated account or VM. This executor is not a sandbox.

## Repository navigation

The workspace menu contains **Overview**, **Repositories**, and **Runners**. The **Selected repository** picker searches by name or URL and keeps the selection while navigating **Links**, **Run history**, **Execution graphs**, and **CI settings**. The selected branch is shown below the picker.

Repository and branch selection are stored in URLs such as `#repository-links?repository=URL&branch=NAME`. They survive navigation, reload, and browser Back/Forward. The browser remembers the selection per account and restores it only while the repository is accessible. Unscoped run history URLs remain available for direct links and workspace activity filters.

The Links view uses `GET /api/v1/repositories/{name}/tree?revision=HASH&path=PATH` to show linked folder contents. The endpoint checks login and repository access and returns immediate nodes with `name`, `kind`, and `is_link`. The root uses an empty `path`; file contents are never downloaded.

## CI settings and execution graphs

### CI settings

**CI settings** provides Visual and TOML editing for the selected repository and branch, dependency edges, change-path preview, server validation, undo/redo, and line-based diffs. Preview and analysis do not commit, push, or run pipelines.

Repositories used as a Lore link's **Source Repository** are read-only in CI settings so their source content remains controlled by the link relationship. When `.lore-ci.toml` is missing, LoreHub offers a starter build template; editing and saving the template creates the configuration in a new revision. New pipeline runs are started from this page after reviewing the selected branch configuration.

![CI settings page](./web/CISettings.png)

### Execution graphs

**Execution graphs** keeps each run's recorded snapshot separate from the current configuration. Historical OS, paths, and stages are never replaced by new settings. Selecting a job shows status, duration, exit code, and paged logs. Wait reasons are derived from server claim evidence; the UI does not invent a dependency blocker when no corresponding run exists.

Read-only execution analysis is available at `GET /api/v1/pipelines/{id}/insights`. It reports related runs, queue and execution timelines, and conservative comparisons with matching previous runs.

Automatic refresh uses a minimum interval of 5 seconds for active runs and busy runners, and 30 seconds for idle lists. Repeated refresh failures increase the interval up to 60 seconds; successful requests restore the normal interval. Hidden tabs and browsers reporting an offline connection pause automatic refresh. Returning to the tab or regaining connectivity immediately checks for due refreshes while preserving normal intervals and failure backoff. Overlapping automatic requests for the same view and scope are skipped. Completed run details stop refreshing automatically; reopen the run to retrieve fresh details. Log paging remains manual, and the main log viewer preserves existing log nodes as new output arrives.

## Repository links

Links connect **Source → Root**. Lore revisions are authoritative for pins; PostgreSQL stores synchronization policy, last success/error, and operation history. Automatic synchronization watches source pushes, while manual synchronization changes Root only when requested. Source-directory creation is enabled by default, and failed Root creation can be retried without reverting the Source commit.

Migration `0024_repository_link_management.sql` is applied when a coordinator starts. The main endpoints are:

| Endpoint | Purpose |
| --- | --- |
| `GET /api/v1/repository-links/summary` | Link counts by accessible Root and branch |
| `POST /api/v1/repositories/{name}/links/policy` | Change branch, path, revision, or auto-update |
| `GET /api/v1/repositories/{name}/link-operations?branch=main` | Recent operations |
| `POST /api/v1/repositories/{name}/link-operations/{id}/retry` | Retry a failed operation |

POST endpoints require CSRF protection. Run `node tests/web-links-preview.mjs` for an isolated local UI fixture.

## Running locally

Requirements are Rust 1.88+, PostgreSQL 18, the Lore CLI, and an accessible Lore server. Workers run on Linux, Windows, and macOS.

```bash
cd /Users/law80/GitHub/lorehub
cargo build --locked
docker compose up -d postgres
cp .env.example .env
set -a
. ./.env
set +a
cargo run --locked -- serve
```

Create a Google OAuth **Web application** client and register `http://127.0.0.1:8080/auth/google/callback`. In another terminal, start a worker with `cargo run --locked -- worker`, then open `http://127.0.0.1:8080/`. Frontend assets are embedded in the Rust binary, so no Node build or separate static server is required.

## Lore services and CLI access

Compose can run DynamoDB + S3 and Local File backends:

```text
lores://127.0.0.1:41337/<repository>   # DynamoDB + S3
lores://127.0.0.1:41338/<repository>   # Local File
```

Configure the corresponding Lore server URL variables in the coordinator environment. Back up `dynamodb_data` together with its S3 payloads; S3 objects alone cannot restore mutable repository state. Existing local repositories need a Lore store migration or a repository-level push.

After login, the Repository page can issue a one-hour CLI access token. Treat it as a bearer credential and keep it out of shell history. Workers issue a short-lived JWT immediately before cloning and pass it to Lore with `--identity-token` and `--access-token`; workers never connect directly to PostgreSQL.

## Pipeline configuration

Place `.lore-ci.toml` at the Lore repository root and push the revision before running it. See [examples/.lore-ci.toml](examples/.lore-ci.toml) and [examples/monorepo.lore-ci.toml](examples/monorepo.lore-ci.toml).

Automatic `[[pipelines]]` routes use exact, case-sensitive paths and `directory/**` patterns. `needs` may reference earlier or same-stage jobs; unknown, duplicate, self, and cyclic dependencies are rejected. Each job runs in its own `/bin/sh -e -c` process, while commands in one job share a shell. Output is bounded and the working directory is removed after completion.

Set `max_parallel_jobs = 4` at the root of a manual configuration, or inside an individual `[[pipelines]]` table, to run independent jobs concurrently on one worker. The allowed range is 1–16 and the default is 1. The Visual editor exposes this as **Maximum parallel jobs**. A job starts only after its `needs` succeed; every stage waits for all jobs in the preceding stages. The first observed failure stops new assignments, cancels running sibling jobs, and waits for their process cleanup before finishing the pipeline. Pending jobs are skipped; interrupted running jobs are failed, or canceled when the pipeline was canceled.

Parallel jobs share the pipeline checkout and working directory. Use separate output paths or explicit `needs` when jobs write the same files. Enable parallelism after updating both the coordinator and workers; older versions reject the new configuration field. Recorded execution graphs include the configured concurrency limit.

## Authentication and access control

`/auth/google/login` starts Google login with the `zenogrid.co.kr` hosted-domain hint. The callback verifies signature, issuer, audience, expiry, nonce, hosted domain, and verified email. The immutable Google `sub` identifies the user.

Repository lists, history, graphs, details, logs, and analysis apply current repository access. Owners, permitted account-group owners and members, and administrators may access a repository. Saving a Sparse View preset does not grant access. Unauthorized details return `404`, unauthorized history cursors return `400`, and cancellation also requires the original requester or an administrator.

## Verification

```bash
node --test tests/*.test.mjs
cargo test --locked --no-default-features --lib
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
```

Database-backed tests use the disposable PostgreSQL wrapper:

```bash
sh scripts/with-test-postgres.sh cargo test --locked --no-default-features --test pipeline_access -- --ignored
```

Preview fixtures for the repository tree, CI editor, execution graphs, and repository links bind to localhost, use in-memory data, and never connect to production Lore or PostgreSQL.

## Operations and administration

The administrator **Operations** page reports queued work, Runner availability, repository checks, and PostgreSQL usage. While an administrator keeps the app visible, operations refresh about every 30 seconds across all pages, with backoff after failures. In-app alerts report queue waits of at least five minutes, expired execution leases, an OS with disconnected Runners and none online, three consecutive repository check failures, and link update errors. A persistent banner links to unresolved issues and the latest 50 issue/recovery events; Mark as read clears the unread count without hiding unresolved issues. Repeated observations do not repeat notifications. Failed requests and repositories missing from the top-100 snapshot are not treated as recovery; the banner counts currently confirmed issues separately from unconfirmed previous observations. Losing administrator access cancels the pending observation request and clears the retained operations state. Alerts are local to the current tab and reset on reload; hidden or closed tabs do not monitor, and no external notifications are sent. This page does not perform automatic retries or cleanup.

Administrators can use **Accounts**, **Account groups**, **Repository access**, **Sparse View**, and **Operations**. These pages and APIs are administrator-only; ordinary users and group members receive `403`. Sparse View presets define which repository files CI fetches when a pipeline sets `sparse_view` in `.lore-ci.toml`; they are not assigned to account groups and do not change repository access or existing local workspaces automatically.

Runner maintenance pauses new assignments while existing work drains. Runner registration, claim, heartbeat, status, and log APIs use the coordinator's JWT-authenticated HTTP API. Claim requests are idempotent by request ID, transient network failures use bounded retries, and automatic updates verify release size and SHA-256 before installation.

## Security notes

Run workers with a dedicated operating-system account or VM. Keep JWT keys, OAuth secrets, database URLs, and CLI tokens out of logs and source control. The shell executor is intended for trusted repository content and does not provide a sandbox.
