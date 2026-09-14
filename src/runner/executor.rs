//! Platform shell executor for trusted repositories, with process-tree teardown.
use std::{path::Path, process::Stdio, time::Duration};

use anyhow::{Context, Result, bail};
#[cfg(unix)]
use nix::{
    sys::signal::{Signal, killpg},
    unistd::Pid,
};
use sqlx::PgPool;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::ci::db;

const LOG_LIMIT: usize = 1024 * 1024;

pub struct Execution<'a> {
    pub pool: &'a PgPool,
    pub pipeline: Uuid,
    pub job: Option<Uuid>,
    pub cancel: &'a CancellationToken,
}

#[cfg(unix)]
struct ProcessTree(Pid);
#[cfg(unix)]
impl ProcessTree {
    fn new(pid: u32) -> Self {
        Self(Pid::from_raw(pid as i32))
    }

    fn terminate(&self) {
        let _ = killpg(self.0, Signal::SIGKILL);
    }
}

#[cfg(windows)]
struct ProcessTree(u32);
#[cfg(windows)]
impl ProcessTree {
    fn new(pid: u32) -> Self {
        Self(pid)
    }

    fn terminate(&self) {
        let _ = std::process::Command::new("taskkill.exe")
            .args(["/PID", &self.0.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

impl Drop for ProcessTree {
    fn drop(&mut self) {
        self.terminate();
    }
}

pub fn command(program: &str, cwd: &Path) -> Command {
    prepare(Command::new(program), cwd)
}

#[cfg(unix)]
pub fn shell(lines: &[String], cwd: &Path) -> Command {
    let mut command = command("/bin/sh", cwd);
    command.args(["-e", "-c", &lines.join("\n")]);
    command
}

#[cfg(windows)]
pub fn shell(lines: &[String], cwd: &Path) -> Command {
    let mut script = String::from("$ErrorActionPreference = 'Stop'\n");
    for line in lines {
        script.push_str("$global:LASTEXITCODE = 0\n");
        script.push_str(line);
        script.push_str("\nif ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\n");
    }
    let mut command = command("powershell.exe", cwd);
    command.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        &script,
    ]);
    command
}

/// Apply the same process and environment policy to shell and VCS commands.
pub fn prepare(mut command: Command, cwd: &Path) -> Command {
    command.current_dir(cwd).env_clear();
    // Do not pass DATABASE_URL or Google OAuth credentials to CI jobs.
    #[cfg(unix)]
    let inherited = ["PATH", "HOME", "TMPDIR", "LANG", "LC_ALL"].as_slice();
    #[cfg(windows)]
    let inherited = [
        "PATH",
        "USERPROFILE",
        "HOME",
        "TEMP",
        "TMP",
        "SystemRoot",
        "ComSpec",
        "PATHEXT",
        "LANG",
        "CARGO_HOME",
        "RUSTUP_HOME",
    ]
    .as_slice();
    for key in inherited {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command
        .env("CI", "true")
        .env("LOREHUB", "true")
        .env("LORE_RUNNER", "true")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    #[cfg(windows)]
    command.creation_flags(0x0000_0200);
    command
}

async fn pump(
    mut reader: impl AsyncRead + Unpin,
    stream: &'static str,
    sender: mpsc::Sender<(&'static str, Vec<u8>)>,
) {
    let mut buffer = [0u8; 8192];
    loop {
        match reader.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if sender.send((stream, buffer[..n].to_vec())).await.is_err() {
                    break;
                }
            }
        }
    }
}

impl Execution<'_> {
    pub async fn run(&self, mut command: Command, timeout: Duration) -> Result<i32> {
        if self.cancel.is_cancelled() {
            bail!("execution canceled");
        }
        let mut child = command.spawn().context("spawn process")?;
        let group = ProcessTree::new(child.id().context("missing process id")?);
        let (sender, mut receiver) = mpsc::channel(16);
        let stdout = tokio::spawn(pump(child.stdout.take().unwrap(), "stdout", sender.clone()));
        let stderr = tokio::spawn(pump(child.stderr.take().unwrap(), "stderr", sender));
        let deadline = tokio::time::sleep(timeout);
        tokio::pin!(deadline);
        let mut exit = None;
        let mut closed = false;
        let mut bytes = 0;
        let mut truncated = false;
        let outcome: Result<i32> = async {
            while exit.is_none() || !closed {
                tokio::select! {
                    biased;
                    _ = self.cancel.cancelled() => bail!("execution canceled or worker lease lost"),
                    _ = &mut deadline => bail!("execution timed out after {} seconds", timeout.as_secs()),
                    status = child.wait(), if exit.is_none() => {
                        exit = Some(status.context("wait for process")?.code().unwrap_or(128));
                        // A successful shell may still leave background children holding pipes.
                        group.terminate();
                    }
                    chunk = receiver.recv(), if !closed => match chunk {
                        None => closed = true,
                        Some((stream, chunk)) => {
                            let count = chunk.len().min(LOG_LIMIT.saturating_sub(bytes));
                            if count > 0 {
                                let content = String::from_utf8_lossy(&chunk[..count]);
                                tokio::time::timeout(Duration::from_secs(5), db::log(self.pool, self.pipeline, self.job, stream, &content)).await??;
                                bytes += count;
                            }
                            if count < chunk.len() && !truncated {
                                tokio::time::timeout(Duration::from_secs(5), db::log(self.pool, self.pipeline, self.job, "system", "[output truncated at 1 MiB]\n")).await??;
                                truncated = true;
                            }
                        }
                    }
                }
            }
            Ok(exit.unwrap())
        }.await;
        drop(group);
        let _ = child.wait().await;
        stdout.abort();
        stderr.abort();
        outcome
    }
}
