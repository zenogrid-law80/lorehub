use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result, ensure};
use sqlx::PgPool;
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{
    executor::{self, Execution},
    update::{SelfUpdater, UpdateCheck},
};
use crate::{
    ci::{
        config::PipelineFile,
        db::{self, Pipeline},
    },
    server::tokens::TokenIssuer,
    vcs::lore,
};

pub struct Worker {
    pub pool: PgPool,
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
        db::register_runner(
            &self.pool,
            self.id,
            &name,
            std::env::consts::OS,
            std::env::consts::ARCH,
            env!("CARGO_PKG_VERSION"),
            docker_available,
        )
        .await?;
        let presence_stop = CancellationToken::new();
        let presence = tokio::spawn({
            let pool = self.pool.clone();
            let id = self.id;
            let stop = presence_stop.clone();
            async move {
                let mut interval = tokio::time::interval(Duration::from_secs(5));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    tokio::select! {
                        _ = stop.cancelled() => break,
                        _ = interval.tick() => match db::touch_runner(&pool, id).await {
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
                db::reap(&self.pool).await?;
                if let Some(pipeline) = db::claim(&self.pool, self.id).await? {
                    self.execute_claimed(pipeline, &shutdown).await?;
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
        if let Err(error) = db::stop_runner(&self.pool, self.id).await {
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
            let pool = self.pool.clone();
            let id = pipeline.id;
            let worker = self.id;
            let cancel = cancel.clone();
            let stop = stop_heartbeat.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(5));
                loop {
                    tokio::select! {
                        _ = stop.cancelled() => break,
                        _ = interval.tick() => {
                            let active = tokio::time::timeout(Duration::from_secs(5), db::heartbeat(&pool, id, worker)).await;
                            if !matches!(active, Ok(Ok(true))) {
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
        let result = db::finish(&self.pool, pipeline.id, self.id, status, error.as_deref()).await;
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
            pool: &self.pool,
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
        let clone = executor::prepare(
            lore::clone_command(
                &self.lore_bin,
                &pipeline.repository_url,
                &pipeline.revision,
                &checkout,
                sparse_view_path.as_deref(),
                token.as_deref(),
            ),
            &root,
        );
        let clone_message = pipeline.sparse_view_name.as_deref().map_or_else(
            || "Cloning the requested Lore revision\n".to_owned(),
            |view| format!("Cloning the requested Lore revision with sparse view {view}\n"),
        );
        db::log(&self.pool, pipeline.id, None, "system", &clone_message).await?;
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
        ensure!(
            sparse_view == pipeline.sparse_view_name.as_deref(),
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
        let jobs = db::create_jobs(&self.pool, pipeline.id, self.id, &config).await?;
        for (job, spec) in jobs.iter().zip(&config.jobs) {
            ensure!(!cancel.is_cancelled(), "pipeline canceled");
            db::job_status(&self.pool, job.id, self.id, "running", None).await?;
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
            let execution = Execution {
                job: Some(job.id),
                ..execution
            };
            let code = execution
                .run(shell, Duration::from_secs(spec.timeout_seconds))
                .await?;
            db::job_status(
                &self.pool,
                job.id,
                self.id,
                if code == 0 { "succeeded" } else { "failed" },
                Some(code),
            )
            .await?;
            ensure!(code == 0, "job {} failed with exit code {code}", job.name);
        }
        Ok(())
    }

    async fn worker_access_token(&self, pipeline: &Pipeline) -> Result<Option<String>> {
        let Some(issuer) = &self.token_issuer else {
            return Ok(None);
        };
        let submitter = pipeline
            .submitted_by
            .context("authenticated worker requires a pipeline submitter")?;
        let repository_name = url::Url::parse(&pipeline.repository_url)?
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .filter(|name| !name.is_empty())
            .context("pipeline repository URL requires a repository name")?
            .to_owned();
        let resource_id: String = sqlx::query_scalar(
            "SELECT resource_id FROM lore_resources WHERE name = $1 AND owner_subject = $2",
        )
        .bind(repository_name)
        .bind(submitter.to_string())
        .fetch_optional(&self.pool)
        .await?
        .context("pipeline submitter no longer owns the repository")?;
        Ok(Some(
            issuer
                .issue_worker(&self.id.to_string(), resource_id)?
                .access_token,
        ))
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
