//! Push notifications wake reconciliation; durable branch cursors cover reconnects.
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    process::Stdio,
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::Command,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{repositories::validate_name, tokens::TokenIssuer};
use crate::ci::config::{NamedPipeline, PipelineFile, valid_relative_path};

#[derive(Clone, Debug, sqlx::FromRow)]
struct WatchedRepository {
    resource_id: String,
    name: String,
    owner_subject: String,
    enabled_branches: Vec<String>,
}

fn repository_key(repository: &WatchedRepository) -> String {
    format!(
        "{}:{}:{}:{}",
        repository.resource_id,
        repository.name,
        repository.owner_subject,
        repository.enabled_branches.join("\u{1f}")
    )
}

/// Only repositories with named pipelines opt into automatic execution.
pub async fn run(
    pool: PgPool,
    binary: String,
    server_url: String,
    public_server_url: String,
    tokens: TokenIssuer,
    shutdown: CancellationToken,
) {
    let mut tasks: HashMap<String, tokio::task::JoinHandle<()>> = HashMap::new();
    let mut interval = tokio::time::interval(Duration::from_secs(15));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            _ = interval.tick() => {}
        }
        let repositories = match sqlx::query_as::<_, WatchedRepository>(
            "SELECT resource.resource_id, resource.name, resource.owner_subject, COALESCE(settings.branches, ARRAY['main']::TEXT[]) AS enabled_branches FROM lore_resources resource LEFT JOIN ci_repository_pipeline_branches settings USING(resource_id) WHERE resource.owner_subject IS NOT NULL"
        ).fetch_all(&pool).await {
            Ok(repositories) => repositories,
            Err(error) => { tracing::warn!(%error, "cannot list push trigger repositories"); continue; }
        };
        let keys: HashSet<_> = repositories.iter().map(repository_key).collect();
        tasks.retain(|key, task| {
            if keys.contains(key) && !task.is_finished() {
                true
            } else {
                task.abort();
                false
            }
        });
        for repository in repositories {
            let key = repository_key(&repository);
            if tasks.contains_key(&key)
                || repository.enabled_branches.is_empty()
                || validate_name(&repository.name).is_err()
                || repository.owner_subject.parse::<Uuid>().is_err()
            {
                continue;
            }
            let pool = pool.clone();
            let binary = binary.clone();
            let url = format!("{}/{}", server_url.trim_end_matches('/'), repository.name);
            let public_url = format!(
                "{}/{}",
                public_server_url.trim_end_matches('/'),
                repository.name
            );
            let tokens = tokens.clone();
            let stop = shutdown.clone();
            tasks.insert(key, tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = stop.cancelled() => break,
                        result = watch(&pool, &binary, &url, &public_url, &tokens, &repository) => {
                            if let Err(error) = result {
                                tracing::warn!(repository = %repository.name, %error, "push trigger reconnecting; cursor retained");
                            }
                        }
                    }
                    tokio::select! {
                        _ = stop.cancelled() => break,
                        _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                    }
                }
            }));
        }
    }
    for (_, task) in tasks {
        task.abort();
        let _ = task.await;
    }
}

fn command(binary: &str, token: &str, repository: Option<&Path>) -> Command {
    let mut command = Command::new(binary);
    command.args([
        "--json",
        "--non-interactive",
        "--no-pager",
        "--identity-token",
        token,
        "--access-token",
        token,
    ]);
    if let Some(repository) = repository {
        command
            .arg("--repository")
            .arg(".")
            .arg("--remote")
            .current_dir(repository);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    command
}

async fn json(mut command: Command) -> Result<(i64, Vec<Value>)> {
    tokio::time::timeout(Duration::from_secs(60), async {
        let mut child = command.spawn().context("start Lore trigger command")?;
        let mut output = Vec::new();
        child
            .stdout
            .take()
            .context("Lore stdout")?
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut output)
            .await?;
        ensure!(
            output.len() <= 4 * 1024 * 1024,
            "Lore trigger output exceeds 4 MiB"
        );
        let status = child.wait().await?;
        let events = parse_events(std::str::from_utf8(&output)?)?;
        let code = events
            .iter()
            .rev()
            .find(|e| e["tagName"] == "complete")
            .and_then(|e| e["data"]["status"].as_i64())
            .context("Lore completion status missing")?;
        ensure!(
            status.success() == (code == 0),
            "Lore process and completion status disagree"
        );
        Ok((code, events))
    })
    .await
    .context("Lore trigger command timed out")?
}

fn parse_events(output: &str) -> Result<Vec<Value>> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).context("invalid Lore JSON event"))
        .collect()
}

#[derive(Debug)]
struct RemoteBranchHead {
    id: String,
    name: String,
    revision: String,
}

fn remote_branch_heads(events: &[Value]) -> Result<Vec<RemoteBranchHead>> {
    events
        .iter()
        .filter(|event| {
            event["tagName"] == "branchListEntry"
                && event["data"]["location"] == "remote"
                && event["data"]["archived"] != true
        })
        .map(|event| {
            let data = &event["data"];
            let id = data["id"].as_str().context("branch ID missing")?;
            let name = data["name"]
                .as_str()
                .filter(|name| !name.is_empty())
                .context("branch name missing")?;
            let revision = data["latest"].as_str().context("branch head missing")?;
            ensure!(valid_hash(revision), "invalid branch revision");
            Ok(RemoteBranchHead {
                id: id.to_owned(),
                name: name.to_owned(),
                revision: revision.to_ascii_lowercase(),
            })
        })
        .collect()
}

fn canonical_branch_policy(configured: &[String], branches: &[RemoteBranchHead]) -> Vec<String> {
    let mut names = configured
        .iter()
        .map(|value| {
            branches
                .iter()
                .find(|branch| branch.name == *value || branch.id == *value)
                .map_or_else(|| value.clone(), |branch| branch.name.clone())
        })
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

async fn success(command: Command) -> Result<Vec<Value>> {
    let (code, events) = json(command).await?;
    ensure!(code == 0, "Lore trigger command failed with status {code}");
    Ok(events)
}

async fn watch(
    pool: &PgPool,
    binary: &str,
    url: &str,
    public_url: &str,
    tokens: &TokenIssuer,
    repository: &WatchedRepository,
) -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let checkout = workspace.path().join("repository");
    let token = tokens
        .issue_worker("push-trigger", repository.resource_id.clone())?
        .access_token;
    let mut clone = command(binary, &token, None);
    clone
        .args(["clone", "--", url])
        .arg(&checkout)
        .current_dir(workspace.path());
    success(clone).await?;

    // Renew the short-lived token and subscription every four minutes.
    let mut subscribe = command(binary, &token, Some(&checkout));
    subscribe.args(["notification", "subscribe", "240"]);
    let mut child = subscribe.spawn()?;
    let mut lines = BufReader::new(child.stdout.take().context("notification stdout")?).lines();
    let expiry = tokio::time::sleep(Duration::from_secs(240));
    tokio::pin!(expiry);
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let reconcile = tokio::select! {
            _ = &mut expiry => break,
            _ = interval.tick() => true,
            line = lines.next_line() => {
                let Some(line) = line? else { break; };
                // Lore prints subscription lifecycle text alongside its JSON events.
                if !line.starts_with('{') { continue; }
                let event: Value = serde_json::from_str(&line).context("invalid notification JSON")?;
                matches!(event["tagName"].as_str(), Some("notificationBranchPushed" | "notificationBranchCreated" | "notificationSubscribed"))
            }
        };
        if reconcile {
            reconcile_repository(pool, binary, public_url, &token, &checkout, repository).await?;
        }
    }
    child.kill().await.ok();
    child.wait().await.ok();
    Ok(())
}

async fn reconcile_repository(
    pool: &PgPool,
    binary: &str,
    public_url: &str,
    token: &str,
    checkout: &Path,
    repository: &WatchedRepository,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    // Take the lock before reading remote heads, so two coordinators cannot
    // overwrite a newer cursor with a stale remote snapshot.
    let locked: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(&repository.resource_id)
            .fetch_one(&mut *tx)
            .await?;
    if !locked {
        return Ok(());
    }
    let mut enabled_branches: Vec<String> = sqlx::query_scalar(
        "SELECT branches FROM ci_repository_pipeline_branches WHERE resource_id=$1",
    )
    .bind(&repository.resource_id)
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or_else(|| vec!["main".into()]);
    let initialized: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM ci_watched_repositories WHERE resource_id = $1)",
    )
    .bind(&repository.resource_id)
    .fetch_one(&mut *tx)
    .await?;
    let mut list = command(binary, token, Some(checkout));
    list.args(["branch", "list"]);
    let events = success(list).await?;
    let remote_branches = remote_branch_heads(&events)?;
    let canonical_branches = canonical_branch_policy(&enabled_branches, &remote_branches);
    if canonical_branches != enabled_branches {
        sqlx::query("UPDATE ci_repository_pipeline_branches SET branches=$2,updated_at=now() WHERE resource_id=$1")
            .bind(&repository.resource_id)
            .bind(&canonical_branches)
            .execute(&mut *tx)
            .await?;
        enabled_branches = canonical_branches;
    }
    for table in [
        "ci_pipeline_routes",
        "ci_pipeline_route_snapshots",
        "ci_branch_cursors",
    ] {
        sqlx::query(&format!(
            "DELETE FROM {table} WHERE resource_id=$1 AND NOT(branch=ANY($2))"
        ))
        .bind(&repository.resource_id)
        .bind(&enabled_branches)
        .execute(&mut *tx)
        .await?;
    }
    let enabled_branch_set: HashSet<&str> = enabled_branches.iter().map(String::as_str).collect();
    for remote_branch in remote_branches
        .iter()
        .filter(|branch| enabled_branch_set.contains(branch.name.as_str()))
    {
        let branch = remote_branch.name.as_str();
        let revision = remote_branch.revision.as_str();
        let mut previous: Option<String> = sqlx::query_scalar(
            "SELECT revision FROM ci_branch_cursors WHERE resource_id = $1 AND branch = $2",
        )
        .bind(&repository.resource_id)
        .bind(branch)
        .fetch_optional(&mut *tx)
        .await?;
        let routes_current: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM ci_pipeline_route_snapshots WHERE resource_id = $1 AND branch = $2 AND revision = $3)",
        )
        .bind(&repository.resource_id)
        .bind(branch)
        .bind(revision)
        .fetch_one(&mut *tx)
        .await?;
        // A branch first pushed after monitoring began is compared with the empty tree.
        if previous.is_none() && initialized {
            previous = Some("0".repeat(64));
        }
        if previous.as_deref() == Some(revision) && routes_current {
            continue;
        }
        let config = if revision.bytes().all(|byte| byte == b'0') {
            None
        } else {
            read_pipeline_config(binary, token, checkout, revision).await?
        };
        sync_routes(
            &mut tx,
            &repository.resource_id,
            public_url,
            branch,
            revision,
            config.as_ref(),
        )
        .await?;
        if let Some(previous) = previous.as_deref() {
            if !revision.bytes().all(|b| b == b'0') {
                let mut changes = Vec::new();
                let first_push = previous.bytes().all(|b| b == b'0');
                if !first_push {
                    let mut diff = command(binary, token, Some(checkout));
                    diff.args(["revision", "diff", previous, "--target", revision]);
                    changes = changed_paths(&success(diff).await?)?;
                }
                if let Some(config) = config.as_ref() {
                    if first_push {
                        // Lore CLI cannot resolve the zero revision. For the first
                        // push, treat each configured path present in the tree as added.
                        let mut checked = HashSet::new();
                        for pattern in config.pipelines.iter().flat_map(|p| &p.changes) {
                            let path = pattern.strip_suffix("/**").unwrap_or(pattern);
                            if !checked.insert(path) {
                                continue;
                            }
                            let mut info = command(binary, token, Some(checkout));
                            info.args(["file", "info", "--revision", revision, "--", path]);
                            let (code, events) = json(info).await?;
                            ensure!(
                                code == 0 || code == 3,
                                "reading initial path failed with Lore status {code}"
                            );
                            if code == 0
                                && events.iter().any(|e| {
                                    e["tagName"] == "fileInfo" && e["data"]["flagDeleted"] != true
                                })
                            {
                                changes.push(path.to_owned());
                            }
                        }
                    }
                    enqueue(
                        &mut tx,
                        &repository.resource_id,
                        public_url,
                        branch,
                        previous,
                        revision,
                        repository.owner_subject.parse()?,
                        config,
                        &changes,
                    )
                    .await?;
                }
            }
        } else {
            tracing::info!(repository = %repository.name, %branch, %revision, "push trigger baseline recorded");
        }
        sqlx::query("INSERT INTO ci_branch_cursors (resource_id, branch, revision) VALUES ($1,$2,$3) ON CONFLICT (resource_id,branch) DO UPDATE SET revision = EXCLUDED.revision, updated_at = now()")
            .bind(&repository.resource_id).bind(branch).bind(revision).execute(&mut *tx).await?;
    }
    sqlx::query(
        "INSERT INTO ci_watched_repositories (resource_id) VALUES ($1) ON CONFLICT DO NOTHING",
    )
    .bind(&repository.resource_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn read_pipeline_config(
    binary: &str,
    token: &str,
    checkout: &Path,
    revision: &str,
) -> Result<Option<PipelineFile>> {
    let config_dir = tempfile::tempdir()?;
    let config_path = config_dir.path().join(".lore-ci.toml");
    let mut read = command(binary, token, Some(checkout));
    read.args([
        "file",
        "write",
        "--path",
        ".lore-ci.toml",
        "--revision",
        revision,
        "--output",
    ])
    .arg(&config_path);
    let (code, events) = json(read).await?;
    if code == 3 {
        return Ok(None);
    }
    let message = events
        .iter()
        .rev()
        .find(|event| event["tagName"] == "complete")
        .and_then(|event| event["data"]["error"]["message"].as_str())
        .unwrap_or("unknown Lore error");
    ensure!(
        code == 0,
        "reading push CI config failed with Lore status {code}: {message}"
    );
    let mut source = String::new();
    tokio::fs::File::open(&config_path)
        .await?
        .take(256 * 1024 + 1)
        .read_to_string(&mut source)
        .await?;
    Ok(Some(PipelineFile::parse(&source)?))
}

pub(crate) fn pipeline_graph_definition(pipeline: &NamedPipeline) -> Result<String> {
    Ok(serde_json::to_string(&serde_json::json!({
        "sparse_view": pipeline.sparse_view,
        "stages": pipeline.stages.iter().map(|stage| serde_json::json!({
            "name": stage,
            "jobs": pipeline.jobs.iter().filter(|job| &job.stage == stage).map(|job| &job.name).collect::<Vec<_>>()
        })).collect::<Vec<_>>()
    }))?)
}

pub(crate) async fn sparse_view_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    resource_id: &str,
    requested_name: Option<&str>,
) -> Result<(Option<String>, Option<String>)> {
    let Some(requested_name) = requested_name else {
        return Ok((None, None));
    };
    let views: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT name, mode, rules FROM sparse_workspace_views WHERE resource_id=$1 AND lower(name)=lower($2) ORDER BY id",
    )
    .bind(resource_id)
    .bind(requested_name)
    .fetch_all(&mut **tx)
    .await?;
    ensure!(
        views.len() == 1,
        "sparse view {requested_name} must resolve to exactly one view for this repository"
    );
    let (name, mode, rules) = views.into_iter().next().unwrap();
    ensure!(
        mode == "sparse",
        "pipeline view {name} must use sparse mode"
    );
    Ok((Some(name), Some(rules)))
}

async fn sync_routes(
    tx: &mut Transaction<'_, Postgres>,
    resource_id: &str,
    url: &str,
    branch: &str,
    revision: &str,
    config: Option<&PipelineFile>,
) -> Result<()> {
    sqlx::query("DELETE FROM ci_pipeline_routes WHERE resource_id = $1 AND branch = $2")
        .bind(resource_id)
        .bind(branch)
        .execute(&mut **tx)
        .await?;
    if let Some(config) = config {
        for pipeline in &config.pipelines {
            sqlx::query("INSERT INTO ci_pipeline_routes (resource_id, repository_url, branch, revision, pipeline_name, runner_os, trigger_patterns, working_directory, graph_definition) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)")
                .bind(resource_id)
                .bind(url)
                .bind(branch)
                .bind(revision)
                .bind(&pipeline.name)
                .bind(&pipeline.runner_os)
                .bind(&pipeline.changes)
                .bind(&pipeline.working_directory)
                .bind(pipeline_graph_definition(pipeline)?)
                .execute(&mut **tx)
                .await?;
        }
    }
    sqlx::query("INSERT INTO ci_pipeline_route_snapshots (resource_id, branch, revision) VALUES ($1,$2,$3) ON CONFLICT (resource_id,branch) DO UPDATE SET revision = EXCLUDED.revision, updated_at = now()")
        .bind(resource_id)
        .bind(branch)
        .bind(revision)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn changed_paths(events: &[Value]) -> Result<Vec<String>> {
    let mut paths = HashSet::new();
    for event in events.iter().filter(|e| e["tagName"] == "revisionDiffFile") {
        for key in ["path", "fromPath"] {
            let path = event["data"][key].as_str().context("diff path missing")?;
            if path.is_empty() && key == "fromPath" {
                continue;
            }
            ensure!(
                valid_relative_path(path),
                "invalid repository-relative diff path"
            );
            paths.insert(path.to_owned());
        }
    }
    Ok(paths.into_iter().collect())
}

#[allow(clippy::too_many_arguments)]
pub async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    resource_id: &str,
    url: &str,
    branch: &str,
    previous: &str,
    revision: &str,
    owner: Uuid,
    config: &PipelineFile,
    changes: &[String],
) -> Result<usize> {
    let mut count = 0;
    for pipeline in config.pipelines.iter().filter(|p| p.matches(changes)) {
        let (sparse_view_name, sparse_view_rules) =
            sparse_view_snapshot(tx, resource_id, pipeline.sparse_view.as_deref()).await?;
        let trigger_patterns: Vec<_> = pipeline
            .changes
            .iter()
            .filter(|pattern| {
                changes.iter().any(|path| {
                    pattern
                        .strip_suffix("/**")
                        .map_or(path == *pattern, |directory| {
                            path == directory
                                || path
                                    .strip_prefix(directory)
                                    .is_some_and(|tail| tail.starts_with('/'))
                        })
                })
            })
            .cloned()
            .collect();
        let matching_paths: Vec<_> = changes
            .iter()
            .filter(|path| {
                trigger_patterns.iter().any(|pattern| {
                    pattern
                        .strip_suffix("/**")
                        .map_or(*path == pattern, |directory| {
                            *path == directory
                                || path
                                    .strip_prefix(directory)
                                    .is_some_and(|tail| tail.starts_with('/'))
                        })
                })
            })
            .cloned()
            .collect();
        let changed_path_count = i32::try_from(matching_paths.len())?;
        let changed_paths: Vec<_> = matching_paths.into_iter().take(32).collect();
        let graph_definition = pipeline_graph_definition(pipeline)?;
        let result = sqlx::query("INSERT INTO pipelines (id, repository_url, revision, submitted_by, pipeline_name, runner_os, branch, previous_revision, trigger_patterns, changed_paths, changed_path_count, working_directory, graph_definition, sparse_view_name, sparse_view_rules) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15) ON CONFLICT DO NOTHING")
            .bind(Uuid::new_v4()).bind(url).bind(revision).bind(owner)
            .bind(&pipeline.name).bind(&pipeline.runner_os).bind(branch).bind(previous)
            .bind(&trigger_patterns).bind(&changed_paths).bind(changed_path_count).bind(&pipeline.working_directory).bind(graph_definition)
            .bind(sparse_view_name).bind(sparse_view_rules)
            .execute(&mut **tx).await?;
        count += result.rows_affected() as usize;
    }
    if count > 0 {
        tracing::info!(repository = url, %revision, count, "push pipelines queued");
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[sqlx::test]
    #[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
    async fn reconciliation_recovers_and_commits_cursor_with_matching_pipelines(pool: PgPool) {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let owner = Uuid::new_v4();
        sqlx::query("INSERT INTO users (id,google_sub,email) VALUES ($1,'trigger-owner','trigger@example.test')")
            .bind(owner).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO lore_resources (resource_id,name,owner_subject) VALUES ('test-resource','example',$1)")
            .bind(owner.to_string()).execute(&pool).await.unwrap();
        let repository = WatchedRepository {
            resource_id: "test-resource".into(),
            name: "example".into(),
            owner_subject: owner.to_string(),
            enabled_branches: vec!["main-id".into()],
        };
        sqlx::query("INSERT INTO ci_repository_pipeline_branches(resource_id,branches) VALUES ('test-resource',ARRAY['main-id'])")
            .execute(&pool).await.unwrap();
        let bin = root.path().join("lore-fixture");
        std::fs::write(
            &bin,
            r#"#!/bin/sh
set -eu
[ "$1" = --json ]
[ "$4" = --identity-token ]
[ "$6" = --access-token ]
[ "$8" = --repository ]
cd "$9"
shift 9
[ "$1" = --remote ]
shift
case "$1 $2" in
  'branch list') cat heads.json ;;
  'revision diff')
    echo "$3" > last-diff-source
    if [ -f fail-diff ]; then printf '%s\n' '{"tagName":"complete","data":{"status":6}}'; exit 6; fi
    cat changes.json ;;
  'file write')
    [ "$3" = --path ]
    [ "$4" = .lore-ci.toml ]
    [ "$5" = --revision ]
    [ "$7" = --output ]
    cp config.toml "$8" ;;
  'file info')
    [ "$3" = --revision ]
    [ "$5" = -- ]
    if [ "$6" = Client ]; then
      printf '%s\n' '{"tagName":"fileInfo","data":{"path":"Client","isDir":true,"flagDeleted":false}}'
    else
      printf '%s\n' '{"tagName":"complete","data":{"status":3}}'
      exit 3
    fi ;;
  *) exit 99 ;;
esac
printf '\n%s\n' '{"tagName":"complete","data":{"status":0}}'
"#,
        )
        .unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o700)).unwrap();
        let heads = |branch: &str, hash: char| {
            std::fs::write(root.path().join("heads.json"), serde_json::json!({"tagName":"branchListEntry","data":{"location":"remote","id":format!("{branch}-id"),"name":branch,"latest":hash.to_string().repeat(64),"archived":false}}).to_string()).unwrap();
        };
        let source = include_str!("../../examples/monorepo.lore-ci.toml");
        std::fs::write(root.path().join("config.toml"), source).unwrap();
        std::fs::write(root.path().join("changes.json"), serde_json::json!({"tagName":"revisionDiffFile","data":{"path":"Server/main.rs","fromPath":"Client/main.rs"}}).to_string()).unwrap();
        let reconcile = || {
            reconcile_repository(
                &pool,
                bin.to_str().unwrap(),
                "lores://localhost/example",
                "fixture-token",
                root.path(),
                &repository,
            )
        };
        heads("main", 'a');
        reconcile().await.unwrap(); // Baseline creates no pipelines.
        let configured: Vec<String> = sqlx::query_scalar(
            "SELECT branches FROM ci_repository_pipeline_branches WHERE resource_id='test-resource'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(configured, ["main"]);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM pipelines")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        heads("main", 'b');
        std::fs::write(root.path().join("fail-diff"), "").unwrap();
        assert!(reconcile().await.is_err());
        let cursor: String =
            sqlx::query_scalar("SELECT revision FROM ci_branch_cursors WHERE branch='main'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(cursor, "a".repeat(64));
        std::fs::remove_file(root.path().join("fail-diff")).unwrap();
        reconcile().await.unwrap();
        reconcile().await.unwrap(); // Repeated notification is idempotent.
        let names: Vec<String> =
            sqlx::query_scalar("SELECT pipeline_name FROM pipelines ORDER BY pipeline_name")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(names, ["client", "server"]);
        heads("main", 'c');
        std::fs::write(root.path().join("changes.json"), serde_json::json!({"tagName":"revisionDiffFile","data":{"path":"README.md","fromPath":""}}).to_string()).unwrap();
        reconcile().await.unwrap();
        let cursor: String =
            sqlx::query_scalar("SELECT revision FROM ci_branch_cursors WHERE branch='main'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(cursor, "c".repeat(64));
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM pipelines")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 2);
        // Branches outside the repository policy are not tracked or executed.
        heads("new-branch", 'd');
        std::fs::write(root.path().join("changes.json"), serde_json::json!({"tagName":"revisionDiffFile","data":{"path":"Client/main.cs","fromPath":""}}).to_string()).unwrap();
        reconcile().await.unwrap();
        let cursor: Option<String> =
            sqlx::query_scalar("SELECT revision FROM ci_branch_cursors WHERE branch='new-branch'")
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert!(cursor.is_none());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM pipelines")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 2);
    }
    #[test]
    fn includes_both_sides_of_renames_and_deleted_directories() {
        let events = parse_events(concat!(
            "{\"tagName\":\"revisionDiffFile\",\"data\":{\"path\":\"Server/a.rs\",\"fromPath\":\"Client/a.rs\",\"action\":\"rename\"}}\n",
            "{\"tagName\":\"revisionDiffFile\",\"data\":{\"path\":\"Client\",\"fromPath\":\"\",\"action\":\"delete\"}}\n",
            "{\"tagName\":\"complete\",\"data\":{\"status\":0}}\n"
        )).unwrap();
        let mut paths = changed_paths(&events).unwrap();
        paths.sort();
        assert_eq!(paths, ["Client", "Client/a.rs", "Server/a.rs"]);
        assert!(changed_paths(&[serde_json::json!({"tagName":"revisionDiffFile","data":{"path":"../escape","fromPath":""}})]).is_err());
    }
}
