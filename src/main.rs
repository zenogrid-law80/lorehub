use std::{
    fs::OpenOptions,
    io::{ErrorKind, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use lorehub::{
    ci::{config::PipelineFile, db},
    runner::Worker,
    server,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    /// Run the HTTP coordinator with Google Workspace authentication.
    Serve {
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
        #[arg(long, env = "GOOGLE_CLIENT_ID", hide_env_values = true)]
        google_client_id: String,
        #[arg(long, env = "GOOGLE_CLIENT_SECRET", hide_env_values = true)]
        google_client_secret: String,
        #[arg(
            long,
            env = "LOREHUB_PUBLIC_URL",
            default_value = "http://127.0.0.1:8080"
        )]
        public_url: String,
        #[arg(long, env = "LORE_BIN", default_value = "lore")]
        lore_bin: String,
        #[arg(
            long,
            env = "LORE_SERVER_URL",
            default_value = "lores://127.0.0.1:41337"
        )]
        lore_server_url: String,
        #[arg(long, env = "LORE_SERVER_PUBLIC_URL")]
        lore_server_public_url: Option<String>,
        #[arg(
            long,
            env = "LORE_JWT_PRIVATE_KEY",
            default_value = "deploy/secrets/lore-jwt-private.pem"
        )]
        lore_jwt_private_key: PathBuf,
        #[arg(
            long,
            env = "LORE_JWT_JWKS",
            default_value = "deploy/secrets/lore-jwks.json"
        )]
        lore_jwt_jwks: PathBuf,
        #[arg(
            long,
            env = "LOREHUB_RUNNER_RELEASES_DIR",
            default_value = "deploy/runner-releases"
        )]
        runner_releases_dir: PathBuf,
        #[arg(long, env = "LOREHUB_AUTH_BIND", default_value = "127.0.0.1:8081")]
        auth_bind: SocketAddr,
        #[arg(long, env = "LOREHUB_BIND", default_value = "127.0.0.1:8080")]
        bind: SocketAddr,
    },
    /// Run one trusted shell worker. Start more processes for pipeline concurrency.
    Worker {
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
        #[arg(long, env = "LOREHUB_WORK_DIR", default_value = ".work")]
        work_dir: PathBuf,
        #[arg(long, env = "LORE_BIN", default_value = "lore")]
        lore_bin: String,
        #[arg(
            long,
            env = "LOREHUB_PUBLIC_URL",
            default_value = "https://lorehub.zenogrid.co.kr"
        )]
        public_url: String,
        #[arg(
            long,
            env = "LORE_JWT_PRIVATE_KEY",
            default_value = "deploy/secrets/lore-jwt-private.pem"
        )]
        lore_jwt_private_key: PathBuf,
        #[arg(
            long,
            env = "LORE_JWT_JWKS",
            default_value = "deploy/secrets/lore-jwks.json"
        )]
        lore_jwt_jwks: PathBuf,
        /// Process at most one queued pipeline and exit (also exits if the queue is empty).
        #[arg(long)]
        once: bool,
    },
    /// Validate a pipeline definition without connecting to PostgreSQL or Lore.
    Validate {
        #[arg(default_value = ".lore-ci.toml")]
        path: PathBuf,
    },
    /// Apply embedded PostgreSQL migrations and exit.
    Migrate {
        #[arg(long, env = "DATABASE_URL", hide_env_values = true)]
        database_url: String,
    },
    /// Configure the Windows scheduled task after MSI file installation.
    #[command(hide = true)]
    InstallWindowsRunner {
        #[arg(long)]
        install_directory: PathBuf,
        #[arg(long)]
        data_directory: PathBuf,
    },
    /// Remove the Windows scheduled task before MSI file removal.
    #[command(hide = true)]
    UninstallWindowsRunner,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "lorehub=info".into()),
        )
        .init();
    let cli = Cli::parse();
    let shutdown = CancellationToken::new();
    install_shutdown_handler(shutdown.clone());
    match cli.command {
        Action::Serve {
            database_url,
            google_client_id,
            google_client_secret,
            public_url,
            lore_bin,
            lore_server_url,
            lore_server_public_url,
            lore_jwt_private_key,
            lore_jwt_jwks,
            runner_releases_dir,
            auth_bind,
            bind,
        } => {
            server::serve(
                &database_url,
                server::ServeConfig {
                    google_client_id,
                    google_client_secret,
                    public_url,
                    lore_bin,
                    lore_server_url,
                    lore_server_public_url,
                    lore_jwt_private_key,
                    lore_jwt_jwks,
                    runner_releases_dir,
                    auth_bind,
                    bind,
                },
                shutdown,
            )
            .await?;
        }
        Action::Worker {
            database_url,
            work_dir,
            lore_bin,
            public_url,
            lore_jwt_private_key,
            lore_jwt_jwks,
            once,
        } => {
            let pool = db::connect_existing(&database_url).await?;
            let id = persistent_runner_id(&work_dir)?;
            let tokens = server::tokens::TokenIssuer::from_files(
                lore_jwt_private_key,
                lore_jwt_jwks,
                &public_url,
                server::auth::ALLOWED_DOMAIN,
            )?;
            let outcome = Worker {
                pool,
                id,
                work_dir,
                lore_bin,
                token_issuer: Some(tokens),
            }
            .run(shutdown, once)
            .await?;
            if outcome == lorehub::runner::WorkerExit::Updated {
                std::process::exit(lorehub::runner::UPDATE_EXIT_CODE);
            }
        }
        Action::Validate { path } => {
            let file = PipelineFile::parse(&std::fs::read_to_string(path)?)?;
            if file.pipelines.is_empty() {
                let (config, _, _, _) = file.select(None)?;
                println!(
                    "Valid: {} stages, {} jobs",
                    config.stages.len(),
                    config.jobs.len()
                );
            } else {
                for pipeline in file.pipelines {
                    println!(
                        "Valid: {} ({}, {}), {} jobs",
                        pipeline.name,
                        pipeline.runner_os,
                        pipeline.working_directory,
                        pipeline.jobs.len()
                    );
                }
            }
        }
        Action::Migrate { database_url } => {
            db::connect(&database_url).await?;
        }
        Action::InstallWindowsRunner {
            install_directory,
            data_directory,
        } => configure_windows_runner(&install_directory, &data_directory, false)?,
        Action::UninstallWindowsRunner => {
            configure_windows_runner(Path::new(""), Path::new(""), true)?
        }
    }
    Ok(())
}

fn configure_windows_runner(
    install_directory: &Path,
    data_directory: &Path,
    uninstall: bool,
) -> Result<()> {
    #[cfg(windows)]
    {
        let script = if uninstall {
            std::env::current_exe()?
                .parent()
                .context("find Windows Runner install directory")?
                .join("uninstall-runner.ps1")
        } else {
            install_directory.join("configure-runner.ps1")
        };
        let mut command = std::process::Command::new("powershell.exe");
        command.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ]);
        command.arg(&script);
        if !uninstall {
            command
                .arg("-InstallDirectory")
                .arg(install_directory)
                .arg("-DataDirectory")
                .arg(data_directory);
        }
        let status = command
            .status()
            .with_context(|| format!("run {}", script.display()))?;
        anyhow::ensure!(
            status.success(),
            "{} failed with {status}",
            script.display()
        );
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = (install_directory, data_directory, uninstall);
        anyhow::bail!("Windows Runner installation is only available on Windows")
    }
}

fn install_shutdown_handler(shutdown: CancellationToken) {
    tokio::spawn({
        async move {
            #[cfg(unix)]
            {
                let mut terminate =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("install SIGTERM handler");
                tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
            }
            #[cfg(windows)]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
            shutdown.cancel();
        }
    });
}

fn persistent_runner_id(work_dir: &Path) -> Result<Uuid> {
    if let Ok(value) = std::env::var("LOREHUB_RUNNER_ID") {
        return value.parse().context("LOREHUB_RUNNER_ID must be a UUID");
    }
    std::fs::create_dir_all(work_dir).context("create runner work directory")?;
    let path = work_dir.join(".runner-id");
    match std::fs::read_to_string(&path) {
        Ok(value) => value.trim().parse().context("parse persisted runner id"),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let id = Uuid::new_v4();
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    writeln!(file, "{id}").context("persist runner id")?;
                    Ok(id)
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    std::fs::read_to_string(&path)?
                        .trim()
                        .parse()
                        .context("parse concurrently persisted runner id")
                }
                Err(error) => Err(error).context("create runner id file"),
            }
        }
        Err(error) => Err(error).context("read runner id file"),
    }
}
