use std::{ffi::OsString, process::Stdio, time::Duration};

use anyhow::{Context, Result, ensure};
use serde::Serialize;
use serde_json::Value;
use tempfile::TempDir;
use tokio::io::AsyncReadExt;
use tokio::{process::Command, time::timeout};
use url::Url;

use crate::ci::config::PipelineFile;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct RepositoryService {
    binary: OsString,
    server_url: String,
    public_server_url: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Repository {
    pub id: String,
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Branch {
    #[serde(skip_serializing)]
    pub(crate) id: String,
    pub name: String,
    pub revision: String,
}

#[derive(Debug)]
pub struct CommandError {
    pub message: String,
}

impl RepositoryService {
    pub fn new(
        binary: impl Into<OsString>,
        server_url: &str,
        public_server_url: &str,
    ) -> Result<Self> {
        let parsed = Url::parse(server_url).context("parse LORE_SERVER_URL")?;
        ensure!(
            matches!(parsed.scheme(), "lore" | "lores"),
            "Lore server URL must use lore or lores"
        );
        ensure!(
            parsed.host_str().is_some(),
            "Lore server URL requires a host"
        );
        ensure!(
            matches!(parsed.path(), "" | "/"),
            "Lore server URL cannot contain a repository path"
        );
        ensure!(
            parsed.username().is_empty() && parsed.password().is_none(),
            "Lore server URL cannot contain credentials"
        );
        ensure!(
            parsed.query().is_none() && parsed.fragment().is_none(),
            "Lore server URL cannot contain a query or fragment"
        );
        let server_url = server_url.trim_end_matches('/').to_owned();
        let public_server_url = public_server_url.trim_end_matches('/').to_owned();
        let public = Url::parse(&public_server_url).context("parse LORE_SERVER_PUBLIC_URL")?;
        ensure!(public.scheme() == "lores", "public Lore URL must use lores");
        Ok(Self {
            binary: binary.into(),
            server_url,
            public_server_url,
        })
    }

    pub fn public_server_url(&self) -> &str {
        &self.public_server_url
    }

    pub fn public_repository_url(&self, name: &str) -> String {
        format!("{}/{}", self.public_server_url, name)
    }

    pub async fn list(&self, access_token: &str) -> Result<Vec<Repository>, CommandError> {
        let output = self
            .run(
                ["repository", "list", self.server_url.as_str()],
                None,
                access_token,
            )
            .await?;
        let mut repositories = parse_repositories(&output, &self.public_server_url)?;
        repositories.sort_by_key(|repository| repository.name.to_lowercase());
        Ok(repositories)
    }

    pub async fn create(
        &self,
        name: &str,
        description: Option<&str>,
        access_token: &str,
    ) -> Result<Repository, CommandError> {
        validate_name(name).map_err(|error| CommandError {
            message: error.to_string(),
        })?;
        if let Some(description) = description {
            validate_description(description).map_err(|error| CommandError {
                message: error.to_string(),
            })?;
        }
        let url = self.public_repository_url(name);
        let command_url = self.command_repository_url(name);
        let workspace = TempDir::new().map_err(internal_error)?;
        let mut args = vec!["repository", "create", command_url.as_str()];
        if let Some(description) = description.filter(|value| !value.trim().is_empty()) {
            args.extend(["--description", description.trim()]);
        }
        let output = self.run(args, Some(&workspace), access_token).await?;
        parse_created_repository(&output, &url).ok_or_else(|| CommandError {
            message: "Lore did not return the created repository".into(),
        })
    }

    pub async fn delete(&self, name: &str, access_token: &str) -> Result<(), CommandError> {
        validate_name(name).map_err(|error| CommandError {
            message: error.to_string(),
        })?;
        let url = self.command_repository_url(name);
        self.run(["repository", "delete", url.as_str()], None, access_token)
            .await?;
        Ok(())
    }

    pub async fn branches(
        &self,
        name: &str,
        access_token: &str,
    ) -> Result<Vec<Branch>, CommandError> {
        validate_name(name).map_err(|error| CommandError {
            message: error.to_string(),
        })?;
        let workspace = TempDir::new().map_err(internal_error)?;
        let url = self.command_repository_url(name);
        self.run(
            [
                "repository",
                "clone",
                "--bare",
                "--",
                url.as_str(),
                "repository",
            ],
            Some(&workspace),
            access_token,
        )
        .await?;
        let output = self
            .run(
                ["--repository", "repository", "--remote", "branch", "list"],
                Some(&workspace),
                access_token,
            )
            .await?;
        let mut branches = parse_branches(&output)?;
        branches.sort_by(|left, right| {
            (left.name != "main", left.name.to_lowercase())
                .cmp(&(right.name != "main", right.name.to_lowercase()))
        });
        Ok(branches)
    }

    /// Reads the CI file through Lore so selection always reflects the immutable revision.
    pub async fn pipeline_file(
        &self,
        name: &str,
        revision: &str,
        access_token: &str,
    ) -> Result<Option<PipelineFile>, CommandError> {
        validate_name(name).map_err(|error| CommandError {
            message: error.to_string(),
        })?;
        if revision.len() != 64 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(CommandError {
                message: "revision must be a full 64-character Lore revision hash".into(),
            });
        }
        let workspace = TempDir::new().map_err(internal_error)?;
        let url = self.command_repository_url(name);
        self.run(
            ["clone", "--", url.as_str(), "repository"],
            Some(&workspace),
            access_token,
        )
        .await?;
        let path = workspace.path().join(".lore-ci.toml");
        let mut command = Command::new(&self.binary);
        command
            .args(["--json", "--non-interactive", "--no-pager"])
            .args([
                "--identity-token",
                access_token,
                "--access-token",
                access_token,
            ])
            .args([
                "--repository",
                "repository",
                "--remote",
                "file",
                "write",
                "--path",
                ".lore-ci.toml",
                "--revision",
                revision,
                "--output",
            ])
            .arg(&path)
            .current_dir(workspace.path())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let output = timeout(COMMAND_TIMEOUT, command.output())
            .await
            .map_err(|_| CommandError {
                message: "Lore repository operation timed out".into(),
            })?
            .map_err(internal_error)?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let completion_status = json_events(&stdout).ok().and_then(|events| {
            events.into_iter().rev().find_map(|event| {
                (event["tagName"] == "complete")
                    .then(|| event["data"]["status"].as_i64())
                    .flatten()
            })
        });
        if completion_status == Some(3) {
            return Ok(None);
        }
        if !output.status.success() || completion_status != Some(0) {
            return Err(CommandError {
                message: lore_error_message(&stdout)
                    .or_else(|| first_line(&String::from_utf8_lossy(&output.stderr)))
                    .unwrap_or_else(|| "unable to read .lore-ci.toml".into()),
            });
        }
        let mut source = String::new();
        tokio::fs::File::open(path)
            .await
            .map_err(internal_error)?
            .take(256 * 1024 + 1)
            .read_to_string(&mut source)
            .await
            .map_err(internal_error)?;
        PipelineFile::parse(&source)
            .map(Some)
            .map_err(|error| CommandError {
                message: format!("invalid .lore-ci.toml: {error}"),
            })
    }

    fn command_repository_url(&self, name: &str) -> String {
        format!("{}/{}", self.server_url, name)
    }

    async fn run<I, S>(
        &self,
        args: I,
        workspace: Option<&TempDir>,
        access_token: &str,
    ) -> Result<String, CommandError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let mut command = Command::new(&self.binary);
        command.args(["--json", "--non-interactive", "--no-pager"]);
        if !access_token.is_empty() {
            command.args([
                "--identity-token",
                access_token,
                "--access-token",
                access_token,
            ]);
        }
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(workspace) = workspace {
            command.current_dir(workspace.path());
        }
        let output = timeout(COMMAND_TIMEOUT, command.output())
            .await
            .map_err(|_| CommandError {
                message: "Lore repository operation timed out".into(),
            })?
            .map_err(internal_error)?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        if !output.status.success() {
            let detail = lore_error_message(&stdout)
                .or_else(|| first_line(&String::from_utf8_lossy(&output.stderr)))
                .unwrap_or_else(|| "Lore repository operation failed".into());
            tracing::warn!(status = ?output.status.code(), %detail, "Lore repository command failed");
            return Err(CommandError { message: detail });
        }
        Ok(stdout)
    }
}

pub fn validate_name(name: &str) -> Result<()> {
    ensure!(
        (1..=100).contains(&name.len()),
        "repository name must be 1 to 100 characters"
    );
    ensure!(
        name.is_ascii(),
        "repository name must contain ASCII characters only"
    );
    ensure!(
        name.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric()),
        "repository name must start with a letter or number"
    );
    ensure!(
        name.chars()
            .last()
            .is_some_and(|c| c.is_ascii_alphanumeric()),
        "repository name must end with a letter or number"
    );
    ensure!(
        name.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')),
        "repository name may contain letters, numbers, dots, dashes, and underscores"
    );
    ensure!(
        !name.contains(".."),
        "repository name cannot contain consecutive dots"
    );
    Ok(())
}

fn validate_description(description: &str) -> Result<()> {
    ensure!(
        description.chars().count() <= 500,
        "description must be at most 500 characters"
    );
    ensure!(
        !description.chars().any(char::is_control),
        "description cannot contain control characters"
    );
    Ok(())
}

fn parse_repositories(output: &str, server_url: &str) -> Result<Vec<Repository>, CommandError> {
    let mut result = Vec::new();
    for event in json_events(output)? {
        if event.get("tagName").and_then(Value::as_str) != Some("repositoryListEntry") {
            continue;
        }
        let data = &event["data"];
        let name = data["name"].as_str().ok_or_else(parse_error)?.to_owned();
        let id = value_string(&data["id"]).ok_or_else(parse_error)?;
        result.push(Repository {
            id,
            url: format!("{server_url}/{name}"),
            name,
        });
    }
    Ok(result)
}

fn parse_created_repository(output: &str, url: &str) -> Option<Repository> {
    json_events(output).ok()?.into_iter().find_map(|event| {
        if event.get("tagName")?.as_str()? != "repositoryCreate" {
            return None;
        }
        let data = &event["data"];
        Some(Repository {
            id: value_string(&data["id"])?,
            name: data["name"].as_str()?.to_owned(),
            url: url.to_owned(),
        })
    })
}

fn parse_branches(output: &str) -> Result<Vec<Branch>, CommandError> {
    let mut result = Vec::new();
    for event in json_events(output)? {
        if event.get("tagName").and_then(Value::as_str) != Some("branchListEntry") {
            continue;
        }
        let data = &event["data"];
        if data["location"].as_str() != Some("remote") || data["archived"].as_bool() == Some(true) {
            continue;
        }
        let id = data["id"].as_str().ok_or_else(parse_error)?.to_owned();
        let name = data["name"]
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(parse_error)?
            .to_owned();
        let revision = data["latest"]
            .as_str()
            .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or_else(parse_error)?
            .to_ascii_lowercase();
        result.push(Branch { id, name, revision });
    }
    Ok(result)
}

fn json_events(output: &str) -> Result<Vec<Value>, CommandError> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(|_| parse_error()))
        .collect()
}

fn value_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| (!value.is_null()).then(|| value.to_string()))
}

fn lore_error_message(output: &str) -> Option<String> {
    output
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find_map(|event| {
            if event.get("tagName")?.as_str()? != "log" {
                return None;
            }
            event["data"]["message"]
                .as_str()?
                .lines()
                .next()
                .map(str::to_owned)
        })
}

fn first_line(value: &str) -> Option<String> {
    value
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
}

fn parse_error() -> CommandError {
    CommandError {
        message: "Lore returned an invalid response".into(),
    }
}

fn internal_error(error: impl std::fmt::Display) -> CommandError {
    tracing::error!(%error, "Lore repository command could not run");
    CommandError {
        message: "Lore repository service is unavailable".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[tokio::test]
    async fn reads_pipeline_file_at_the_requested_revision() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let binary = root.path().join("lore");
        std::fs::write(
            &binary,
            r#"#!/bin/sh
set -eu
[ "$1" = --json ]
[ "$4" = --identity-token ]
[ "$6" = --access-token ]
case "$8" in
  clone)
    [ "$9" = -- ]
    mkdir -p "${11}"
    ;;
  --repository)
    [ "$9" = repository ]
    [ "${10}" = --remote ]
    [ "${11} ${12}" = 'file write' ]
    [ "${13}" = --path ]
    [ "${14}" = .lore-ci.toml ]
    [ "${15}" = --revision ]
    [ "${17}" = --output ]
    cat > "${18}" <<'EOF'
[[pipelines]]
name = "build"
runner_os = "linux"
changes = ["src/**"]
working_directory = "."
stages = ["test"]
[[pipelines.jobs]]
name = "check"
stage = "test"
script = ["true"]
EOF
    ;;
  *) exit 99 ;;
esac
printf '%s\n' '{"tagName":"complete","data":{"status":0}}'
"#,
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let service =
            RepositoryService::new(binary, "lores://server.test", "lores://server.test").unwrap();
        let file = service
            .pipeline_file("project", &"a".repeat(64), "test-token")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(file.pipelines.len(), 1);
        assert_eq!(file.pipelines[0].name, "build");
    }

    #[test]
    fn validates_names_and_server_urls() {
        for valid in ["engine", "project-2", "tools.core", "asset_store"] {
            validate_name(valid).unwrap();
        }
        for invalid in ["", ".hidden", "trail-", "two..dots", "a/b", "한글"] {
            assert!(validate_name(invalid).is_err(), "{invalid}");
        }
        assert!(
            RepositoryService::new("lore", "lores://server:41337", "lores://server:41337").is_ok()
        );
        assert_eq!(
            RepositoryService::new("lore", "lore://server:41337", "lores://server:41337")
                .unwrap()
                .public_server_url(),
            "lores://server:41337"
        );
        assert!(RepositoryService::new("lore", "https://server", "lores://server").is_err());
        assert!(
            RepositoryService::new("lore", "lores://server/repository", "lores://server").is_err()
        );
    }

    #[test]
    fn parses_lore_json_events() {
        let output = concat!(
            "{\"tagName\":\"repositoryListEntry\",\"data\":{\"id\":\"abc123\",\"name\":\"engine\"}}\n",
            "{\"tagName\":\"complete\",\"data\":{\"status\":0}}\n"
        );
        assert_eq!(
            parse_repositories(output, "lores://server:41337").unwrap(),
            vec![Repository {
                id: "abc123".into(),
                name: "engine".into(),
                url: "lores://server:41337/engine".into()
            }]
        );
    }

    #[test]
    fn parses_active_remote_branches() {
        let output = format!(
            concat!(
                "{{\"tagName\":\"branchListEntry\",\"data\":{{\"location\":\"local\",\"id\":\"draft-id\",\"name\":\"draft\",\"latest\":\"{}\",\"archived\":false}}}}\n",
                "{{\"tagName\":\"branchListEntry\",\"data\":{{\"location\":\"remote\",\"id\":\"main-id\",\"name\":\"main\",\"latest\":\"{}\",\"archived\":false}}}}\n",
                "{{\"tagName\":\"branchListEntry\",\"data\":{{\"location\":\"remote\",\"id\":\"old-id\",\"name\":\"old\",\"latest\":\"{}\",\"archived\":true}}}}\n"
            ),
            "a".repeat(64),
            "B".repeat(64),
            "c".repeat(64),
        );
        assert_eq!(
            parse_branches(&output).unwrap(),
            vec![Branch {
                id: "main-id".into(),
                name: "main".into(),
                revision: "b".repeat(64),
            }]
        );
    }
}
