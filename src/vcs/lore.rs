//! Lore CLI arguments live here; execution policy belongs to the runner.
use std::path::Path;

use tokio::process::Command;

pub fn version_command(binary: &str) -> Command {
    let mut command = Command::new(binary);
    command.arg("--version");
    command
}

pub fn clone_command(
    binary: &str,
    repository_url: &str,
    revision: &str,
    destination: &Path,
    view: Option<&Path>,
    access_token: Option<&str>,
) -> Command {
    let mut command = Command::new(binary);
    command.args(["--non-interactive", "--no-pager"]);
    if let Some(token) = access_token {
        command.args(["--identity-token", token, "--access-token", token]);
    }
    command.args(["clone", "--revision", revision]);
    if let Some(view) = view {
        command.arg("--view").arg(view);
    }
    command.args(["--", repository_url]);
    command.arg(destination);
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_command_passes_sparse_view_before_repository_separator() {
        let command = clone_command(
            "lore",
            "lores://example.test/project",
            &"a".repeat(64),
            Path::new("checkout"),
            Some(Path::new("pipeline.view")),
            None,
        );
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            [
                "--non-interactive",
                "--no-pager",
                "clone",
                "--revision",
                &"a".repeat(64),
                "--view",
                "pipeline.view",
                "--",
                "lores://example.test/project",
                "checkout",
            ]
        );
    }
}
