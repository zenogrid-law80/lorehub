use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result, ensure};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{
    CoordinatorClient,
    client::retryable,
    executor::{self, Execution},
    update::{SelfUpdater, UpdateCheck},
};
use crate::{
    ci::{config::PipelineFile, db::Pipeline},
    server::tokens::TokenIssuer,
    vcs::lore,
};

pub struct Worker {
    pub coordinator: CoordinatorClient,
    pub id: Uuid,
    pub work_dir: PathBuf,
    pub lore_bin: String,
    pub token_issuer: Option<TokenIssuer>,
}

pub const UPDATE_EXIT_CODE: i32 = 75;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerExit {
    Stopped,
    Updated,
}

impl Worker {
    pub async fn run(&self, shutdown: CancellationToken, once: bool) -> Result<WorkerExit> {
        tokio::fs::create_dir_all(&self.work_dir).await?;
        let updater = if once {
            None
        } else {
            self.token_issuer
                .as_ref()
                .map(SelfUpdater::from_environment)
                .transpose()?
                .flatten()
        };
        // Fail before claiming work if the required VCS executable is unavailable.
        let version = tokio::time::timeout(
            Duration::from_secs(10),
            executor::prepare(lore::version_command(&self.lore_bin), &self.work_dir).output(),
        )
        .await??;
        ensure!(version.status.success(), "Lore CLI is unavailable");
        let name = runner_name(self.id);
        let docker_available = detect_docker().await;
        let mut retry_delay = Duration::from_secs(2);
        loop {
            let registration = tokio::select! {
                _ = shutdown.cancelled() => return Ok(WorkerExit::Stopped),
                result = self.coordinator.register(&name, docker_available) => result,
            };
            match registration {
                Ok(()) => break,
                Err(error) if !once && retryable(&error) => {
                    tracing::warn!(%error, "runner registration unavailable; waiting to reconnect");
                    if !wait_to_reconnect(&shutdown, &mut retry_delay).await {
                        return Ok(WorkerExit::Stopped);
                    }
                }
                Err(error) => return Err(error),
            }
        }
        let presence_stop = CancellationToken::new();
        let presence = tokio::spawn({
            let coordinator = self.coordinator.clone();
            let id = self.id;
            let stop = presence_stop.clone();
            async move {
                let mut interval = tokio::time::interval(Duration::from_secs(5));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    tokio::select! {
                        _ = stop.cancelled() => break,
                        _ = interval.tick() => match coordinator.touch().await {
                            Ok(true) => {}
                            Ok(false) => tracing::warn!(runner_id = %id, "runner registration disappeared"),
                            Err(error) => tracing::warn!(runner_id = %id, %error, "runner heartbeat failed"),
                        }
                    }
                }
            }
        });
        tracing::info!(worker_id = %self.id, runner_name = %name, os = std::env::consts::OS, arch = std::env::consts::ARCH, docker_available, "worker started");
        let mut next_update_check = tokio::time::Instant::now();
        let mut worker_exit = WorkerExit::Stopped;
        let mut claim_request = Uuid::new_v4();
        retry_delay = Duration::from_secs(2);
        let outcome: Result<()> = async {
            loop {
                if shutdown.is_cancelled() {
                    break;
                }
                if let (Some(updater), Some(issuer)) = (&updater, &self.token_issuer)
                    && tokio::time::Instant::now() >= next_update_check
                {
                    next_update_check = tokio::time::Instant::now() + updater.interval;
                    let update = tokio::select! {
                        _ = shutdown.cancelled() => break,
                        update = updater.check_and_install(self.id, issuer) => update,
                    };
                    match update {
                        Ok(UpdateCheck::Installed(version)) => {
                            tracing::info!(%version, "runner update installed; requesting service restart");
                            worker_exit = WorkerExit::Updated;
                            break;
                        }
                        Ok(UpdateCheck::Current | UpdateCheck::Unavailable) => {}
                        Err(error) => {
                            tracing::warn!(%error, "runner self-update check failed; continuing with current version");
                        }
                    }
                }
                let claim = tokio::select! {
                    _ = shutdown.cancelled() => break,
                    result = self.coordinator.claim_with_request(claim_request) => result,
                };
                let pipeline = match claim {
                    Ok(pipeline) => pipeline,
                    Err(error) if !once && retryable(&error) => {
                        tracing::warn!(%error, %claim_request, "work polling unavailable; waiting to reconnect");
                        if !wait_to_reconnect(&shutdown, &mut retry_delay).await { break; }
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                claim_request = Uuid::new_v4();
                retry_delay = Duration::from_secs(2);
                if let Some(pipeline) = pipeline {
                    let id = pipeline.id;
                    if let Err(error) = self.execute_claimed(pipeline, &shutdown).await {
                        if once || !retryable(&error) { return Err(error); }
                        // Never execute a pipeline again to retry its completion report.
                        // New claims remain blocked while its lease is still active.
                        tracing::warn!(pipeline_id = %id, %error, "completion report not acknowledged; continuing work polling");
                    }
                    if once {
                        break;
                    }
                } else {
                    if once {
                        break;
                    }
                    tokio::select! {
                        _ = shutdown.cancelled() => break,
                        _ = tokio::time::sleep(Duration::from_secs(2)) => {}
                    }
                }
            }
            Ok(())
        }
        .await;
        presence_stop.cancel();
        if let Err(error) = presence.await {
            tracing::warn!(%error, "runner heartbeat task failed");
        }
        if let Err(error) = self.coordinator.stop().await {
            tracing::warn!(runner_id = %self.id, %error, "failed to record runner shutdown");
        }
        outcome?;
        Ok(worker_exit)
    }

    pub async fn execute_claimed(
        &self,
        pipeline: Pipeline,
        shutdown: &CancellationToken,
    ) -> Result<()> {
        let cancel = shutdown.child_token();
        let stop_heartbeat = CancellationToken::new();
        let heartbeat = {
            let coordinator = self.coordinator.clone();
            let id = pipeline.id;
            let cancel = cancel.clone();
            let stop = stop_heartbeat.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(5));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                let mut consecutive_failures = 0;
                loop {
                    tokio::select! {
                        _ = stop.cancelled() => break,
                        _ = interval.tick() => {
                            match tokio::time::timeout(Duration::from_secs(5), coordinator.heartbeat(id)).await {
                                Ok(Ok(true)) => consecutive_failures = 0,
                                Ok(Ok(false)) => {
                                    tracing::warn!(pipeline_id = %id, "pipeline lease is no longer active");
                                    cancel.cancel();
                                    break;
                                }
                                Ok(Err(error)) => {
                                    consecutive_failures += 1;
                                    tracing::warn!(pipeline_id = %id, consecutive_failures, %error, "pipeline heartbeat failed");
                                }
                                Err(_) => {
                                    consecutive_failures += 1;
                                    tracing::warn!(pipeline_id = %id, consecutive_failures, "pipeline heartbeat timed out");
                                }
                            }
                            if consecutive_failures >= 3 {
                                tracing::warn!(pipeline_id = %id, "canceling pipeline after repeated heartbeat failures");
                                cancel.cancel();
                                break;
                            }
                        }
                    }
                }
            })
        };
        tracing::info!(pipeline_id = %pipeline.id, "pipeline started");
        let outcome = self.execute(&pipeline, &cancel).await;
        let error = outcome.as_ref().err().map(|e| format!("{e:#}"));
        let status = if outcome.is_ok() {
            "succeeded"
        } else {
            "failed"
        };
        let result = self
            .coordinator
            .finish(pipeline.id, status, error.as_deref())
            .await;
        stop_heartbeat.cancel();
        heartbeat.await?;
        result?;
        tracing::info!(pipeline_id = %pipeline.id, %status, ?error, "worker finished pipeline");
        Ok(())
    }

    async fn execute(&self, pipeline: &Pipeline, cancel: &CancellationToken) -> Result<()> {
        let workspace = tempfile::Builder::new()
            .prefix(&format!("{}-", pipeline.id))
            .tempdir_in(&self.work_dir)?;
        let root = workspace.path().canonicalize()?;
        let checkout = root.join("source");
        let execution = Execution {
            coordinator: &self.coordinator,
            pipeline: pipeline.id,
            job: None,
            cancel,
        };
        let token = self.worker_access_token(pipeline).await?;
        let sparse_view_path = if let Some(rules) = pipeline.sparse_view_rules.as_deref() {
            let path = root.join("pipeline-sparse.view");
            let mut effective_rules = rules.trim_end().to_owned();
            effective_rules.push_str("\n!/.lore-ci.toml\n");
            tokio::fs::write(&path, effective_rules).await?;
            Some(path)
        } else {
            None
        };
        let dependency_branch = (!pipeline.pipeline_needs.is_empty())
            .then_some(pipeline.branch.as_deref())
            .flatten();
        let clone_source = dependency_branch.map_or(
            lore::CloneSource::Revision(&pipeline.revision),
            lore::CloneSource::Branch,
        );
        let clone = executor::prepare(
            lore::clone_command(
                &self.lore_bin,
                &pipeline.repository_url,
                clone_source,
                &checkout,
                sparse_view_path.as_deref(),
                token.as_deref(),
            ),
            &root,
        );
        let source_description = dependency_branch.map_or_else(
            || "the requested Lore revision".to_owned(),
            |branch| format!("latest Lore branch revision for {branch} after dependencies"),
        );
        let clone_message = pipeline.sparse_view_name.as_deref().map_or_else(
            || format!("Cloning {source_description}\n"),
            |view| format!("Cloning {source_description} with sparse view {view}\n"),
        );
        self.coordinator
            .log(pipeline.id, None, "system", &clone_message)
            .await?;
        ensure!(
            execution.run(clone, Duration::from_secs(300)).await? == 0,
            "Lore clone failed"
        );
        ensure!(!cancel.is_cancelled(), "pipeline canceled");
        let config_path = checkout
            .join(".lore-ci.toml")
            .canonicalize()
            .context("find .lore-ci.toml in requested revision")?;
        ensure!(
            config_path.starts_with(&checkout),
            ".lore-ci.toml must stay inside the checkout"
        );
        ensure!(
            tokio::fs::metadata(&config_path).await?.is_file(),
            ".lore-ci.toml must be a regular file"
        );
        let file = tokio::fs::File::open(config_path).await?;
        let mut source = String::new();
        file.take(256 * 1024 + 1)
            .read_to_string(&mut source)
            .await?;
        let file = PipelineFile::parse(&source)?;
        let (config, working_directory, runner_os, sparse_view) =
            file.select(pipeline.pipeline_name.as_deref())?;
        ensure!(
            runner_os == pipeline.runner_os.as_deref(),
            "queued runner OS differs from revision configuration"
        );
        let sparse_view_matches = sparse_view == pipeline.sparse_view_name.as_deref()
            || (pipeline.sparse_view_name.is_none() && pipeline.sparse_view_rules.is_none());
        ensure!(
            sparse_view_matches,
            "queued sparse view differs from revision configuration"
        );
        ensure!(
            runner_os.is_none_or(|os| os == std::env::consts::OS),
            "pipeline requires a different runner OS"
        );
        let working_directory = checkout
            .join(working_directory)
            .canonicalize()
            .context("find pipeline working_directory")?;
        ensure!(
            working_directory.starts_with(&checkout) && working_directory.is_dir(),
            "working_directory must be a directory inside the checkout"
        );
        let jobs = self.coordinator.create_jobs(pipeline.id, &config).await?;
        for (job, spec) in jobs.iter().zip(&config.jobs) {
            ensure!(!cancel.is_cancelled(), "pipeline canceled");
            self.coordinator.job_status(job.id, "running", None).await?;
            let job_token = self.worker_access_token(pipeline).await?;
            let mut shell = executor::shell(&spec.script, &working_directory);
            // All entries share one platform shell so directory and environment changes persist.
            shell
                .env("LORE_PIPELINE_ID", pipeline.id.to_string())
                .env("LORE_JOB_ID", job.id.to_string())
                .env("LORE_JOB_NAME", &job.name)
                .env("LORE_REVISION", &pipeline.revision)
                .env("LORE_REPOSITORY_URL", &pipeline.repository_url)
                .env("LORE_BIN", &self.lore_bin)
                .env("LORE_PROJECT_DIR", &checkout);
            if let Some(branch) = pipeline.branch.as_deref() {
                shell.env("LORE_BRANCH", branch);
            }
            if let Some(token) = job_token.as_deref() {
                shell
                    .env("LORE_IDENTITY_TOKEN", token)
                    .env("LORE_ACCESS_TOKEN", token);
            }
            for name in ["ECR_ACCESS_KEY_ID", "ECR_ACCESS_KEY", "ECR_REGION"] {
                if let Some(value) = std::env::var_os(name) {
                    shell.env(name, value);
                }
            }
            let execution = Execution {
                job: Some(job.id),
                ..execution
            };
            let code = execution
                .run(shell, Duration::from_secs(spec.timeout_seconds))
                .await?;
            self.coordinator
                .job_status(
                    job.id,
                    if code == 0 { "succeeded" } else { "failed" },
                    Some(code),
                )
                .await?;
            ensure!(code == 0, "job {} failed with exit code {code}", job.name);
        }
        Ok(())
    }

    async fn worker_access_token(&self, pipeline: &Pipeline) -> Result<Option<String>> {
        let Some(_issuer) = &self.token_issuer else {
            return Ok(None);
        };
        Ok(Some(
            self.coordinator.worker_access_token(pipeline.id).await?,
        ))
    }
}

async fn wait_to_reconnect(shutdown: &CancellationToken, delay: &mut Duration) -> bool {
    let wait = *delay + Duration::from_millis(u64::from(rand::random::<u8>()));
    *delay = (*delay * 2).min(Duration::from_secs(30));
    tokio::select! {
        _ = shutdown.cancelled() => false,
        _ = tokio::time::sleep(wait) => true,
    }
}

async fn detect_docker() -> bool {
    for executable in docker_executables() {
        let mut command = tokio::process::Command::new(executable);
        command.arg("--version").kill_on_drop(true);
        if matches!(
            tokio::time::timeout(Duration::from_secs(3), command.output()).await,
            Ok(Ok(output)) if output.status.success()
        ) {
            return true;
        }
    }
    false
}

#[cfg(target_os = "windows")]
fn docker_executables() -> &'static [&'static str] {
    &[
        "docker.exe",
        r"C:\Program Files\Docker\Docker\resources\bin\docker.exe",
    ]
}

#[cfg(target_os = "macos")]
fn docker_executables() -> &'static [&'static str] {
    &[
        "docker",
        "/usr/local/bin/docker",
        "/opt/homebrew/bin/docker",
        "/Applications/Docker.app/Contents/Resources/bin/docker",
    ]
}

#[cfg(all(unix, not(target_os = "macos")))]
fn docker_executables() -> &'static [&'static str] {
    &[
        "docker",
        "/usr/bin/docker",
        "/usr/local/bin/docker",
        "/snap/bin/docker",
    ]
}

#[cfg(not(any(unix, target_os = "windows")))]
fn docker_executables() -> &'static [&'static str] {
    &["docker"]
}

fn runner_name(id: Uuid) -> String {
    ["LOREHUB_RUNNER_NAME", "COMPUTERNAME", "HOSTNAME"]
        .into_iter()
        .filter_map(|key| std::env::var(key).ok())
        .map(|value| value.trim().chars().take(128).collect::<String>())
        .find(|value| !value.is_empty())
        .unwrap_or_else(|| format!("runner-{}", &id.to_string()[..8]))
}
