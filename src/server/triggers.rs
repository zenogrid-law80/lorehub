//! Push notifications wake reconciliation; durable branch cursors cover reconnects.
use std::{
    collections::{BTreeMap, HashMap, HashSet},
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

use super::{
    repositories::{RepositoryService, StorageBackend, validate_name},
    repository_access,
    tokens::TokenIssuer,
};
use crate::ci::config::{NamedPipeline, PipelineFile, valid_relative_path};

#[derive(Clone, Debug, sqlx::FromRow)]
struct WatchedRepository {
    resource_id: String,
    name: String,
    owner_subject: String,
    enabled_branches: Vec<String>,
    storage_backend: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RepositoryLink {
    path: String,
    source_resource_id: String,
    source_branch_id: String,
    source_revision: String,
    tracking: bool,
}

#[derive(Clone, Debug, sqlx::FromRow)]
struct PendingLinkUpdate {
    root_resource_id: String,
    root_name: String,
    root_owner_subject: String,
    root_storage_backend: String,
    root_branch: String,
    source_branch_id: String,
    source_revision: String,
}

fn repository_key(repository: &WatchedRepository) -> String {
    format!(
        "{}:{}:{}:{}:{}",
        repository.resource_id,
        repository.name,
        repository.owner_subject,
        repository.storage_backend,
        repository.enabled_branches.join("\u{1f}")
    )
}

/// Watch every owned repository: CI branch pushes and link-source pushes both
/// need durable reconciliation after notification reconnects.
pub async fn run(
    pool: PgPool,
    binary: String,
    repository_service: RepositoryService,
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
            "SELECT resource.resource_id, resource.name, resource.owner_subject, resource.storage_backend, COALESCE(settings.branches, ARRAY['main']::TEXT[]) AS enabled_branches FROM lore_resources resource LEFT JOIN ci_repository_pipeline_branches settings USING(resource_id) WHERE resource.owner_subject IS NOT NULL"
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
                || validate_name(&repository.name).is_err()
                || repository.owner_subject.parse::<Uuid>().is_err()
            {
                continue;
            }
            let pool = pool.clone();
            let binary = binary.clone();
            let backend = match StorageBackend::parse(&repository.storage_backend) {
                Ok(backend) => backend,
                Err(error) => {
                    tracing::warn!(repository = %repository.name, message = %error.message, "invalid repository storage backend");
                    continue;
                }
            };
            let url = match repository_service.command_repository_url_for(backend, &repository.name)
            {
                Ok(url) => url,
                Err(error) => {
                    tracing::warn!(repository = %repository.name, message = %error.message, "repository storage backend unavailable");
                    continue;
                }
            };
            let public_url = match repository_service
                .public_repository_url_for(backend, &repository.name)
            {
                Ok(url) => url,
                Err(error) => {
                    tracing::warn!(repository = %repository.name, message = %error.message, "repository storage backend unavailable");
                    continue;
                }
            };
            let tokens = tokens.clone();
            let repository_service = repository_service.clone();
            let stop = shutdown.clone();
            tasks.insert(key, tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = stop.cancelled() => break,
                        result = watch(&pool, &binary, &url, &public_url, &repository_service, &tokens, &repository) => {
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
        // Some Lore commands still print a human-readable summary after their
        // JSON events. The completion event below remains mandatory, so it is
        // safe to ignore those non-JSON lifecycle/summary lines here.
        .filter(|line| line.trim_start().starts_with('{'))
        .map(|line| serde_json::from_str(line).context("invalid Lore JSON event"))
        .collect()
}

#[derive(Clone, Debug)]
struct RemoteBranchHead {
    id: String,
    name: String,
    revision: String,
}

fn bounded_identifier<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    let value = value.as_str().with_context(|| format!("{field} missing"))?;
    ensure!(
        !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control),
        "invalid {field}"
    );
    Ok(value)
}

fn repository_resource_id(value: &Value) -> Result<String> {
    let value = bounded_identifier(value, "link repository ID")?;
    let repository_id = value.strip_prefix("urc-").unwrap_or(value);
    ensure!(
        repository_id.len() == 32 && repository_id.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "invalid link repository ID"
    );
    Ok(format!("urc-{}", repository_id.to_ascii_lowercase()))
}

fn repository_links(events: &[Value]) -> Result<Vec<RepositoryLink>> {
    events
        .iter()
        .filter(|event| event["tagName"] == "linkEntry")
        .map(|event| {
            let data = &event["data"];
            let path = data["linkPath"]
                .as_str()
                .filter(|path| valid_relative_path(path) && path.len() <= 4096)
                .context("invalid link path")?;
            let source_resource_id = repository_resource_id(&data["link"])?;
            let source_branch_id = bounded_identifier(&data["branch"], "link branch ID")?;
            let source_revision = data["revision"].as_str().context("link revision missing")?;
            ensure!(valid_hash(source_revision), "invalid link revision");
            let tracking = data["tracking"]
                .as_bool()
                .context("link tracking state missing")?;
            Ok(RepositoryLink {
                path: path.to_owned(),
                source_resource_id,
                source_branch_id: source_branch_id.to_owned(),
                source_revision: source_revision.to_ascii_lowercase(),
                tracking,
            })
        })
        .collect()
}

fn link_change_revision(events: &[Value]) -> Result<Option<String>> {
    let Some(revision) = events
        .iter()
        .rev()
        .find(|event| event["tagName"] == "linkChange")
        .and_then(|event| event["data"]["revision"].as_str())
    else {
        return Ok(None);
    };
    ensure!(valid_hash(revision), "invalid updated link revision");
    if revision.bytes().all(|byte| byte == b'0') {
        Ok(None)
    } else {
        Ok(Some(revision.to_ascii_lowercase()))
    }
}

fn pushed_revision(events: &[Value]) -> Result<String> {
    events
        .iter()
        .rev()
        .find(|event| event["tagName"] == "branchPushRevisionPushEnd")
        .and_then(|event| event["data"]["newRemoteRevision"].as_str())
        .filter(|revision| valid_hash(revision))
        .map(str::to_ascii_lowercase)
        .context("Lore push revision missing")
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
    let message = events
        .iter()
        .rev()
        .find(|event| event["tagName"] == "complete")
        .and_then(|event| event["data"]["error"]["message"].as_str())
        .unwrap_or("unknown Lore error");
    ensure!(
        code == 0,
        "Lore trigger command failed with status {code}: {message}"
    );
    Ok(events)
}

async fn watch(
    pool: &PgPool,
    binary: &str,
    url: &str,
    public_url: &str,
    repository_service: &RepositoryService,
    tokens: &TokenIssuer,
    repository: &WatchedRepository,
) -> Result<()> {
    let workspace = tempfile::tempdir()?;
    let checkout = workspace.path().join("repository");
    // Diffing a root revision that updates links can traverse the linked
    // repositories, so the watcher needs the owner's accessible repository
    // set just like link indexing and updates do.
    let token = owner_worker_token(
        pool,
        tokens,
        "push-trigger",
        &repository.owner_subject,
        &repository.resource_id,
    )
    .await?;
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
            let Some(remote_branches) =
                reconcile_repository(pool, binary, public_url, &token, &checkout, repository)
                    .await?
            else {
                continue;
            };
            if let Err(error) =
                refresh_link_index(pool, binary, url, tokens, repository, &remote_branches).await
            {
                tracing::warn!(repository = %repository.name, %error, "cannot refresh repository link index");
            }
            propagate_link_updates(
                pool,
                binary,
                repository_service,
                tokens,
                repository,
                &remote_branches,
            )
            .await;
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
) -> Result<Option<Vec<RemoteBranchHead>>> {
    let mut tx = pool.begin().await?;
    // Take the lock before reading remote heads, so two coordinators cannot
    // overwrite a newer cursor with a stale remote snapshot.
    let locked: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(&repository.resource_id)
            .fetch_one(&mut *tx)
            .await?;
    if !locked {
        return Ok(None);
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
    Ok(Some(remote_branches))
}

fn link_branch_lock_key(resource_id: &str, branch: &str) -> String {
    format!("repository-links:{resource_id}:{branch}")
}

pub(crate) async fn acquire_link_branch_lock(
    tx: &mut Transaction<'_, Postgres>,
    resource_id: &str,
    branch: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 1))")
        .bind(link_branch_lock_key(resource_id, branch))
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn try_link_branch_lock(
    tx: &mut Transaction<'_, Postgres>,
    resource_id: &str,
    branch: &str,
) -> Result<bool> {
    Ok(
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1, 1))")
            .bind(link_branch_lock_key(resource_id, branch))
            .fetch_one(&mut **tx)
            .await?,
    )
}

async fn owner_worker_token(
    pool: &PgPool,
    tokens: &TokenIssuer,
    worker_id: &str,
    owner_subject: &str,
    root_resource_id: &str,
) -> Result<String> {
    let resources = repository_access::resource_ids(pool, owner_subject).await?;
    ensure!(
        resources
            .iter()
            .any(|resource| resource == root_resource_id),
        "repository owner no longer has access to the root repository"
    );
    Ok(tokens
        .issue_worker_resources(worker_id, resources)?
        .access_token)
}

async fn refresh_link_index(
    pool: &PgPool,
    binary: &str,
    url: &str,
    tokens: &TokenIssuer,
    repository: &WatchedRepository,
    remote_branches: &[RemoteBranchHead],
) -> Result<()> {
    let mut branches: Vec<String> = sqlx::query_scalar(
        "SELECT root_branch FROM repository_link_snapshots WHERE root_resource_id=$1",
    )
    .bind(&repository.resource_id)
    .fetch_all(pool)
    .await?;
    branches.extend(remote_branches.iter().map(|b| b.name.clone()));
    branches.sort();
    branches.dedup();
    for name in branches {
        refresh_link_branch_index(
            pool,
            binary,
            url,
            tokens,
            repository,
            remote_branches,
            &name,
        )
        .await?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn refresh_link_branch_index(
    pool: &PgPool,
    binary: &str,
    url: &str,
    tokens: &TokenIssuer,
    repository: &WatchedRepository,
    remote_branches: &[RemoteBranchHead],
    branch_name: &str,
) -> Result<()> {
    let Some(branch) = remote_branches
        .iter()
        .find(|branch| branch.name == branch_name)
    else {
        let mut tx = pool.begin().await?;
        if try_link_branch_lock(&mut tx, &repository.resource_id, branch_name).await? {
            sqlx::query(
                "DELETE FROM repository_link_snapshots WHERE root_resource_id=$1 AND root_branch=$2",
            )
            .bind(&repository.resource_id)
            .bind(branch_name)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        return Ok(());
    };

    let indexed_revision: Option<String> = sqlx::query_scalar(
        "SELECT root_revision FROM repository_link_snapshots WHERE root_resource_id=$1 AND root_branch=$2",
    )
    .bind(&repository.resource_id)
    .bind(branch_name)
    .fetch_optional(pool)
    .await?;
    if indexed_revision.as_deref() == Some(branch.revision.as_str()) {
        return Ok(());
    }

    let token = owner_worker_token(
        pool,
        tokens,
        "link-index",
        &repository.owner_subject,
        &repository.resource_id,
    )
    .await?;
    let workspace = tempfile::tempdir()?;
    let checkout = workspace.path().join("repository");
    let mut clone = command(binary, &token, None);
    clone
        .args(["clone", "--revision", branch.revision.as_str(), "--", url])
        .arg(&checkout)
        .current_dir(workspace.path());
    success(clone).await?;
    let mut list = command(binary, &token, Some(&checkout));
    list.args(["link", "list"]);
    let links = repository_links(&success(list).await?)?;

    let mut tx = pool.begin().await?;
    if !try_link_branch_lock(&mut tx, &repository.resource_id, branch_name).await? {
        return Ok(());
    }
    let indexed_revision: Option<String> = sqlx::query_scalar(
        "SELECT root_revision FROM repository_link_snapshots WHERE root_resource_id=$1 AND root_branch=$2",
    )
    .bind(&repository.resource_id)
    .bind(branch_name)
    .fetch_optional(&mut *tx)
    .await?;
    if indexed_revision.as_deref() == Some(branch.revision.as_str()) {
        tx.commit().await?;
        return Ok(());
    }
    sqlx::query("INSERT INTO repository_link_snapshots(root_resource_id,root_branch,root_revision) VALUES($1,$2,$3) ON CONFLICT(root_resource_id,root_branch) DO UPDATE SET root_revision=EXCLUDED.root_revision,updated_at=now()")
        .bind(&repository.resource_id)
        .bind(branch_name)
        .bind(&branch.revision)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "DELETE FROM repository_link_dependencies WHERE root_resource_id=$1 AND root_branch=$2",
    )
    .bind(&repository.resource_id)
    .bind(branch_name)
    .execute(&mut *tx)
    .await?;
    for link in &links {
        sqlx::query("INSERT INTO repository_link_dependencies(root_resource_id,root_branch,root_revision,link_path,source_resource_id,source_branch_id,source_revision,tracking) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
            .bind(&repository.resource_id)
            .bind(branch_name)
            .bind(&branch.revision)
            .bind(&link.path)
            .bind(&link.source_resource_id)
            .bind(&link.source_branch_id)
            .bind(&link.source_revision)
            .bind(link.tracking)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    tracing::info!(
        repository = %repository.name,
        branch = branch_name,
        revision = %branch.revision,
        links = links.len(),
        "repository link index refreshed"
    );
    Ok(())
}

async fn dependency_reaches(pool: &PgPool, start: &str, target: &str) -> Result<bool> {
    if start == target {
        return Ok(true);
    }
    Ok(sqlx::query_scalar(
        "WITH RECURSIVE upstream(resource_id) AS (SELECT source_resource_id FROM repository_link_dependencies WHERE root_resource_id=$1 UNION SELECT dependency.source_resource_id FROM repository_link_dependencies dependency JOIN upstream ON dependency.root_resource_id=upstream.resource_id) SELECT EXISTS(SELECT 1 FROM upstream WHERE resource_id=$2)",
    )
    .bind(start)
    .bind(target)
    .fetch_one(pool)
    .await?)
}

async fn propagate_link_updates(
    pool: &PgPool,
    binary: &str,
    repository_service: &RepositoryService,
    tokens: &TokenIssuer,
    source: &WatchedRepository,
    remote_branches: &[RemoteBranchHead],
) {
    let dependencies: Vec<PendingLinkUpdate> = match sqlx::query_as(
        "SELECT dependency.root_resource_id, root.name AS root_name, root.owner_subject AS root_owner_subject, root.storage_backend AS root_storage_backend, dependency.root_branch, dependency.source_branch_id, dependency.source_revision FROM repository_link_dependencies dependency JOIN lore_resources root ON root.resource_id=dependency.root_resource_id LEFT JOIN repository_link_policies policy ON policy.root_resource_id=dependency.root_resource_id AND policy.root_branch=dependency.root_branch AND policy.link_path=dependency.link_path WHERE dependency.source_resource_id=$1 AND root.owner_subject IS NOT NULL AND COALESCE(policy.auto_update,true) ORDER BY dependency.root_resource_id,dependency.root_branch,dependency.link_path",
    )
    .bind(&source.resource_id)
    .fetch_all(pool)
    .await
    {
        Ok(dependencies) => dependencies,
        Err(error) => {
            tracing::warn!(repository = %source.name, %error, "cannot load dependent repository links");
            return;
        }
    };
    let heads: HashMap<_, _> = remote_branches
        .iter()
        .map(|branch| (branch.id.as_str(), branch.revision.as_str()))
        .collect();
    let mut grouped: BTreeMap<(String, String), Vec<(PendingLinkUpdate, String)>> = BTreeMap::new();
    for dependency in dependencies {
        let Some(revision) = heads.get(dependency.source_branch_id.as_str()) else {
            continue;
        };
        if dependency.source_revision.eq_ignore_ascii_case(revision) {
            continue;
        }
        grouped
            .entry((
                dependency.root_resource_id.clone(),
                dependency.root_branch.clone(),
            ))
            .or_default()
            .push((dependency, (*revision).to_owned()));
    }

    for ((root_resource_id, root_branch), updates) in grouped {
        match dependency_reaches(pool, &source.resource_id, &root_resource_id).await {
            Ok(true) => {
                record_link_failure(
                    pool,
                    &root_resource_id,
                    &root_branch,
                    &source.resource_id,
                    "Automatic sync blocked by a repository dependency cycle",
                )
                .await;
                tracing::warn!(
                    source = %source.name,
                    root_resource = %root_resource_id,
                    root_branch,
                    "automatic link update skipped because it would propagate through a dependency cycle"
                );
                continue;
            }
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(source = %source.name, root_resource = %root_resource_id, %error, "cannot check repository link dependency cycle");
                continue;
            }
        }
        if let Err(error) =
            update_root_links(pool, binary, repository_service, tokens, source, &updates).await
        {
            record_link_failure(
                pool,
                &root_resource_id,
                &root_branch,
                &source.resource_id,
                &error.to_string(),
            )
            .await;
            tracing::warn!(
                source = %source.name,
                root = %updates[0].0.root_name,
                branch = %updates[0].0.root_branch,
                %error,
                "automatic repository link update failed"
            );
        }
    }
}

async fn record_link_failure(pool: &PgPool, root: &str, branch: &str, source: &str, error: &str) {
    if let Err(db_error) = sqlx::query("INSERT INTO repository_link_policies(root_resource_id,root_branch,link_path,last_error) SELECT root_resource_id,root_branch,link_path,$4 FROM repository_link_dependencies WHERE root_resource_id=$1 AND root_branch=$2 AND source_resource_id=$3 ON CONFLICT(root_resource_id,root_branch,link_path) DO UPDATE SET last_error=$4,updated_at=now() WHERE repository_link_policies.auto_update")
        .bind(root).bind(branch).bind(source).bind(error).execute(pool).await {
        tracing::warn!(%db_error, "cannot persist link sync failure");
    }
}

async fn update_root_links(
    pool: &PgPool,
    binary: &str,
    repository_service: &RepositoryService,
    tokens: &TokenIssuer,
    source: &WatchedRepository,
    requested: &[(PendingLinkUpdate, String)],
) -> Result<()> {
    let first = requested
        .first()
        .context("missing repository link update")?;
    let root = &first.0;
    let token = owner_worker_token(
        pool,
        tokens,
        "link-update",
        &root.root_owner_subject,
        &root.root_resource_id,
    )
    .await?;
    let backend = StorageBackend::parse(&root.root_storage_backend)
        .map_err(|error| anyhow::anyhow!(error.message))?;

    let mut tx = pool.begin().await?;
    if !try_link_branch_lock(&mut tx, &root.root_resource_id, &root.root_branch).await? {
        return Ok(());
    }
    let current: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT dependency.link_path,source_branch_id,source_revision FROM repository_link_dependencies dependency LEFT JOIN repository_link_policies policy ON policy.root_resource_id=dependency.root_resource_id AND policy.root_branch=dependency.root_branch AND policy.link_path=dependency.link_path WHERE dependency.root_resource_id=$1 AND dependency.root_branch=$2 AND source_resource_id=$3 AND COALESCE(policy.auto_update,true) ORDER BY dependency.link_path",
    )
    .bind(&root.root_resource_id)
    .bind(&root.root_branch)
    .bind(&source.resource_id)
    .fetch_all(&mut *tx)
    .await?;
    let requested_heads: HashMap<&str, &str> = requested
        .iter()
        .map(|(dependency, revision)| (dependency.source_branch_id.as_str(), revision.as_str()))
        .collect();
    let pending: Vec<_> = current
        .into_iter()
        .filter_map(|(path, branch, revision)| {
            requested_heads
                .get(branch.as_str())
                .filter(|desired| !revision.eq_ignore_ascii_case(desired))
                .map(|desired| (path, branch, (*desired).to_owned()))
        })
        .collect();
    if pending.is_empty() {
        tx.commit().await?;
        return Ok(());
    }

    let branches = repository_service
        .branches_on(&root.root_name, backend, &token)
        .await
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let root_head = branches
        .iter()
        .find(|branch| branch.name == root.root_branch)
        .with_context(|| format!("root branch '{}' was not found", root.root_branch))?;
    let indexed_root_revision: Option<String> = sqlx::query_scalar(
        "SELECT root_revision FROM repository_link_snapshots WHERE root_resource_id=$1 AND root_branch=$2",
    )
    .bind(&root.root_resource_id)
    .bind(&root.root_branch)
    .fetch_optional(&mut *tx)
    .await?;
    if indexed_root_revision.as_deref() != Some(root_head.revision.as_str()) {
        // The root moved after its dependency index was built. Drop the stale
        // snapshot and let its watcher rebuild before applying source updates.
        sqlx::query(
            "DELETE FROM repository_link_snapshots WHERE root_resource_id=$1 AND root_branch=$2",
        )
        .bind(&root.root_resource_id)
        .bind(&root.root_branch)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        return Ok(());
    }
    let url = repository_service
        .command_repository_url_for(backend, &root.root_name)
        .map_err(|error| anyhow::anyhow!(error.message))?;
    let workspace = tempfile::tempdir()?;
    let checkout = workspace.path().join("repository");
    let mut clone = command(binary, &token, None);
    clone
        .args([
            "clone",
            "--revision",
            root_head.revision.as_str(),
            "--",
            url.as_str(),
        ])
        .arg(&checkout)
        .current_dir(workspace.path());
    success(clone).await?;

    let mut changed = false;
    let mut applied = Vec::with_capacity(pending.len());
    for (path, branch, desired_revision) in pending {
        let mut update = command(binary, &token, Some(&checkout));
        update.args(["link", "update", "--", path.as_str()]);
        let events = success(update).await?;
        let revision = match link_change_revision(&events)? {
            Some(revision) => {
                changed = true;
                // Lore requires a clean working tree before updating another
                // link. Commit each changed link independently, then push the
                // resulting commit chain once after all updates succeed.
                let message = format!("Update Lore link {path} from {}", source.name);
                let mut commit = command(binary, &token, Some(&checkout));
                commit.args(["commit", message.as_str()]);
                success(commit).await?;
                revision
            }
            None => desired_revision,
        };
        applied.push((path, branch, revision));
    }

    let pushed_root_revision = if changed {
        let mut push = command(binary, &token, Some(&checkout));
        // A concurrent user push must be re-indexed rather than auto-merged
        // over potentially changed link metadata.
        push.arg("push");
        let revision = pushed_revision(&success(push).await?)?;
        tracing::info!(
            source = %source.name,
            root = %root.root_name,
            branch = %root.root_branch,
            revision,
            links = applied.len(),
            "repository links updated and pushed"
        );
        Some(revision)
    } else {
        None
    };

    for (path, branch, revision) in applied {
        sqlx::query("INSERT INTO repository_link_policies(root_resource_id,root_branch,link_path,last_success_at) VALUES($1,$2,$3,now()) ON CONFLICT(root_resource_id,root_branch,link_path) DO UPDATE SET last_success_at=now(),last_error=NULL,updated_at=now()")
            .bind(&root.root_resource_id).bind(&root.root_branch).bind(&path).execute(&mut *tx).await?;
        let update = sqlx::query("UPDATE repository_link_dependencies SET source_revision=$5, root_revision=COALESCE($6,root_revision), updated_at=now() WHERE root_resource_id=$1 AND root_branch=$2 AND link_path=$3 AND source_resource_id=$4 AND source_branch_id=$7")
            .bind(&root.root_resource_id)
            .bind(&root.root_branch)
            .bind(path)
            .bind(&source.resource_id)
            .bind(revision)
            .bind(pushed_root_revision.as_deref())
            .bind(branch);
        update.execute(&mut *tx).await?;
    }
    if pushed_root_revision.is_some() {
        sqlx::query(
            "DELETE FROM repository_link_snapshots WHERE root_resource_id=$1 AND root_branch=$2",
        )
        .bind(&root.root_resource_id)
        .bind(&root.root_branch)
        .execute(&mut *tx)
        .await?;
    }
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
    // Lore CLI versions have reported a missing file as status 3 and 82.
    // A repository without CI configuration is valid and must not prevent
    // independent push work such as repository-link propagation.
    if matches!(code, 3 | 82) {
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
        "pipeline_needs": pipeline.needs,
        "stages": pipeline.stages.iter().map(|stage| serde_json::json!({
            "name": stage,
            "jobs": pipeline.jobs.iter().filter(|job| &job.stage == stage).map(|job| &job.name).collect::<Vec<_>>()
        })).collect::<Vec<_>>(),
        "dependencies": pipeline.jobs.iter().filter(|job| !job.needs.is_empty()).map(|job| serde_json::json!({
            "job": job.name,
            "needs": job.needs
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
    if views.is_empty() {
        tracing::warn!(
            resource = resource_id,
            view = requested_name,
            "pipeline sparse view was not found; continuing without a view"
        );
        return Ok((None, None));
    }
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
            sqlx::query("INSERT INTO ci_pipeline_routes (resource_id, repository_url, branch, revision, pipeline_name, category, runner_os, trigger_patterns, working_directory, graph_definition) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
                .bind(resource_id)
                .bind(url)
                .bind(branch)
                .bind(revision)
                .bind(&pipeline.name)
                .bind(&pipeline.category)
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
        let result = sqlx::query("INSERT INTO pipelines (id, repository_url, revision, submitted_by, pipeline_name, pipeline_needs, category, runner_os, branch, previous_revision, trigger_patterns, changed_paths, changed_path_count, working_directory, graph_definition, sparse_view_name, sparse_view_rules) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17) ON CONFLICT DO NOTHING")
            .bind(Uuid::new_v4()).bind(url).bind(revision).bind(owner)
            .bind(&pipeline.name).bind(&pipeline.needs).bind(&pipeline.category).bind(&pipeline.runner_os).bind(branch).bind(previous)
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

    #[sqlx::test]
    #[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
    async fn manual_link_policy_survives_reindex_and_blocks_automatic_updates(pool: PgPool) {
        let owner = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO users(id,google_sub,email) VALUES($1,'link-owner','link@example.test')",
        )
        .bind(owner)
        .execute(&pool)
        .await
        .unwrap();
        for (resource, name) in [("urc-root", "root"), ("urc-source", "source")] {
            sqlx::query(
                "INSERT INTO lore_resources(resource_id,name,owner_subject) VALUES($1,$2,$3)",
            )
            .bind(resource)
            .bind(name)
            .bind(owner.to_string())
            .execute(&pool)
            .await
            .unwrap();
        }
        let mut tx = pool.begin().await.unwrap();
        super::super::links::save_policy(&mut tx, "urc-root", "release", "Test", false)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        // Re-indexing does not reset user policy, including on non-main branches.
        for _ in 0..2 {
            sqlx::query("DELETE FROM repository_link_snapshots WHERE root_resource_id='urc-root'")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO repository_link_snapshots(root_resource_id,root_branch,root_revision) VALUES('urc-root','release',$1)").bind("a".repeat(64)).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO repository_link_dependencies(root_resource_id,root_branch,root_revision,link_path,source_resource_id,source_branch_id,source_revision,tracking) VALUES('urc-root','release',$1,'Test','urc-source','main-id',$1,true)").bind("a".repeat(64)).execute(&pool).await.unwrap();
        }
        let tokens = TokenIssuer::from_files(
            "tests/fixtures/test-private.pem",
            "tests/fixtures/test-jwks.json",
            "http://localhost:8080",
            "zenogrid.co.kr",
        )
        .unwrap();
        let service = RepositoryService::new(
            "/nonexistent/link-test-lore",
            "lores://localhost:41337",
            "lores://localhost:41337",
        )
        .unwrap();
        let source = WatchedRepository {
            resource_id: "urc-source".into(),
            name: "source".into(),
            owner_subject: owner.to_string(),
            enabled_branches: vec![],
            storage_backend: "dynamodb_s3".into(),
        };
        let requested = [(
            PendingLinkUpdate {
                root_resource_id: "urc-root".into(),
                root_name: "root".into(),
                root_owner_subject: owner.to_string(),
                root_storage_backend: "dynamodb_s3".into(),
                root_branch: "release".into(),
                source_branch_id: "main-id".into(),
                source_revision: "a".repeat(64),
            },
            "b".repeat(64),
        )];
        // If manual mode is ignored, this would try to execute the nonexistent binary.
        update_root_links(
            &pool,
            "/nonexistent/link-test-lore",
            &service,
            &tokens,
            &source,
            &requested,
        )
        .await
        .unwrap();
        record_link_failure(&pool, "urc-root", "release", "urc-source", "test failure").await;
        let row: (bool,Option<String>) = sqlx::query_as("SELECT auto_update,last_error FROM repository_link_policies WHERE root_resource_id='urc-root'").fetch_one(&pool).await.unwrap();
        assert_eq!(row, (false, None));
        sqlx::query("UPDATE repository_link_policies SET auto_update=true")
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            update_root_links(
                &pool,
                "/nonexistent/link-test-lore",
                &service,
                &tokens,
                &source,
                &requested
            )
            .await
            .is_err()
        );
        record_link_failure(&pool, "urc-root", "release", "urc-source", "test failure").await;
        let error: Option<String> = sqlx::query_scalar(
            "SELECT last_error FROM repository_link_policies WHERE root_resource_id='urc-root'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(error.as_deref(), Some("test failure"));
    }

    #[test]
    fn parses_repository_link_events_and_link_updates() {
        let revision = "a".repeat(64);
        let events = vec![serde_json::json!({
            "tagName": "linkEntry",
            "data": {
                "link": "0194b726b34e72b0b45550b88a967076",
                "linkNode": 7,
                "linkPath": "Dependencies/Source",
                "sourceNode": 1,
                "sourcePath": "Libraries/Core",
                "branch": "source-main-branch",
                "tracking": true,
                "revision": revision,
                "flags": 0
            }
        })];
        assert_eq!(
            repository_links(&events).unwrap(),
            [RepositoryLink {
                path: "Dependencies/Source".into(),
                source_resource_id: "urc-0194b726b34e72b0b45550b88a967076".into(),
                source_branch_id: "source-main-branch".into(),
                source_revision: "a".repeat(64),
                tracking: true,
            }]
        );

        let changed = [serde_json::json!({
            "tagName": "linkChange",
            "data": { "revision": "B".repeat(64) }
        })];
        assert_eq!(
            link_change_revision(&changed).unwrap(),
            Some("b".repeat(64))
        );
        let unchanged = [serde_json::json!({
            "tagName": "linkChange",
            "data": { "revision": "0".repeat(64) }
        })];
        assert_eq!(link_change_revision(&unchanged).unwrap(), None);
    }

    #[test]
    fn rejects_unsafe_repository_link_events() {
        let events = [serde_json::json!({
            "tagName": "linkEntry",
            "data": {
                "link": "0194b726b34e72b0b45550b88a967076",
                "linkPath": "../outside",
                "branch": "main",
                "tracking": false,
                "revision": "a".repeat(64)
            }
        })];
        assert!(repository_links(&events).is_err());
    }

    #[sqlx::test]
    #[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
    async fn detects_transitive_repository_link_cycles(pool: PgPool) {
        for (resource, name) in [("urc-a", "a"), ("urc-b", "b"), ("urc-c", "c")] {
            sqlx::query("INSERT INTO lore_resources(resource_id,name) VALUES($1,$2)")
                .bind(resource)
                .bind(name)
                .execute(&pool)
                .await
                .unwrap();
        }
        let revision = "a".repeat(64);
        for resource in ["urc-a", "urc-b"] {
            sqlx::query("INSERT INTO repository_link_snapshots(root_resource_id,root_branch,root_revision) VALUES($1,'main',$2)")
                .bind(resource)
                .bind(&revision)
                .execute(&pool)
                .await
                .unwrap();
        }
        for (root, path, source) in [
            ("urc-a", "Dependencies/B", "urc-b"),
            ("urc-b", "Dependencies/C", "urc-c"),
        ] {
            sqlx::query("INSERT INTO repository_link_dependencies(root_resource_id,root_branch,root_revision,link_path,source_resource_id,source_branch_id,source_revision,tracking) VALUES($1,'main',$2,$3,$4,'main-id',$2,false)")
                .bind(root)
                .bind(&revision)
                .bind(path)
                .bind(source)
                .execute(&pool)
                .await
                .unwrap();
        }
        assert!(dependency_reaches(&pool, "urc-a", "urc-c").await.unwrap());
        assert!(!dependency_reaches(&pool, "urc-c", "urc-a").await.unwrap());
        assert!(dependency_reaches(&pool, "urc-a", "urc-a").await.unwrap());
    }

    #[test]
    fn graph_snapshot_includes_job_dependencies() {
        let source = r#"
[[pipelines]]
name = "build"
runner_os = "linux"
changes = ["src/**"]
working_directory = "."
stages = ["build"]
[[pipelines.jobs]]
name = "compile"
stage = "build"
script = ["cargo check"]
[[pipelines.jobs]]
name = "package"
stage = "build"
needs = ["compile"]
script = ["cargo build"]
"#;
        let file = PipelineFile::parse(source).unwrap();
        let graph: serde_json::Value =
            serde_json::from_str(&pipeline_graph_definition(&file.pipelines[0]).unwrap()).unwrap();
        assert_eq!(graph["dependencies"][0]["job"], "package");
        assert_eq!(graph["dependencies"][0]["needs"][0], "compile");
    }

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
            storage_backend: "dynamodb_s3".into(),
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

    #[test]
    fn parses_json_events_around_human_readable_lore_output() {
        let events = parse_events(concat!(
            "subscription started\n",
            "{\"tagName\":\"complete\",\"data\":{\"status\":0}}\n",
            "No links found in this repository\n"
        ))
        .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["tagName"], "complete");

        assert!(parse_events("{not-json}\n").is_err());
    }
}
