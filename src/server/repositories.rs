pub mod tree;

use std::{
    collections::HashSet,
    ffi::OsString,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tempfile::TempDir;
use tokio::io::AsyncReadExt;
use tokio::{process::Command, time::timeout};
use url::Url;

use crate::ci::config::{PipelineFile, valid_relative_path};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const PIPELINE_FILE_NAME: &str = ".lore-ci.toml";
const PIPELINE_FILE_MAX_BYTES: usize = 256 * 1024;

#[derive(Clone)]
pub struct RepositoryService {
    binary: OsString,
    dynamodb_s3: RepositoryEndpoint,
    local_file: Option<RepositoryEndpoint>,
}

#[derive(Clone)]
struct RepositoryEndpoint {
    server_url: String,
    public_server_url: String,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StorageBackend {
    #[default]
    #[serde(rename = "dynamodb_s3")]
    DynamoDbS3,
    #[serde(rename = "local_file")]
    LocalFile,
}

impl StorageBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DynamoDbS3 => "dynamodb_s3",
            Self::LocalFile => "local_file",
        }
    }

    pub fn parse(value: &str) -> Result<Self, CommandError> {
        match value {
            "dynamodb_s3" => Ok(Self::DynamoDbS3),
            "local_file" => Ok(Self::LocalFile),
            _ => Err(CommandError {
                message: format!("unknown repository storage backend '{value}'"),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Repository {
    pub id: String,
    pub name: String,
    pub url: String,
    pub storage_backend: StorageBackend,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Branch {
    #[serde(skip_serializing)]
    pub(crate) id: String,
    pub name: String,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RepositoryLink {
    pub path: String,
    pub source_repository_id: String,
    pub source_path: String,
    pub source_branch_id: String,
    pub source_revision: String,
    pub tracking: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RepositoryLinks {
    pub branch: String,
    pub revision: String,
    pub links: Vec<RepositoryLink>,
}

pub(crate) fn direct_repository_links<T, F>(links: Vec<T>, path: F) -> Vec<T>
where
    F: for<'a> Fn(&'a T) -> &'a str,
{
    let paths: HashSet<String> = links.iter().map(|link| path(link).to_owned()).collect();
    links
        .into_iter()
        .filter(|link| {
            let link_path = path(link);
            !link_path
                .match_indices('/')
                .any(|(index, _)| paths.contains(&link_path[..index]))
        })
        .collect()
}

pub(crate) fn link_path_is_below(path: &str, parent: &str) -> bool {
    path.strip_prefix(parent)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

enum LinkMutation<'a> {
    Add {
        path: &'a str,
        source_url: &'a str,
        source_path: &'a str,
        source_repository_id: &'a str,
        source_branch_id: &'a str,
        source_revision: &'a str,
        disable_branching: bool,
    },
    Update {
        path: &'a str,
    },
    Remove {
        path: &'a str,
    },
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
            dynamodb_s3: RepositoryEndpoint {
                server_url,
                public_server_url,
            },
            local_file: None,
        })
    }

    pub fn with_local_backend(mut self, server_url: &str, public_server_url: &str) -> Result<Self> {
        let local = Self::new(self.binary.clone(), server_url, public_server_url)?;
        self.local_file = Some(local.dynamodb_s3);
        Ok(self)
    }

    pub fn public_server_url(&self) -> &str {
        &self.dynamodb_s3.public_server_url
    }

    pub fn available_storage_backends(&self) -> Vec<StorageBackend> {
        self.endpoints().map(|(backend, _)| backend).collect()
    }

    pub fn public_repository_url(&self, name: &str) -> String {
        self.public_repository_url_for(StorageBackend::DynamoDbS3, name)
            .expect("primary repository backend is always configured")
    }

    pub fn public_repository_url_for(
        &self,
        backend: StorageBackend,
        name: &str,
    ) -> Result<String, CommandError> {
        Ok(format!(
            "{}/{}",
            self.endpoint(backend)?.public_server_url,
            name
        ))
    }

    pub fn command_repository_url_for(
        &self,
        backend: StorageBackend,
        name: &str,
    ) -> Result<String, CommandError> {
        Ok(format!("{}/{}", self.endpoint(backend)?.server_url, name))
    }

    pub fn storage_backend_for_public_url(
        &self,
        name: &str,
        repository_url: &str,
    ) -> Option<StorageBackend> {
        self.endpoints().find_map(|(backend, endpoint)| {
            (repository_url == format!("{}/{}", endpoint.public_server_url, name))
                .then_some(backend)
        })
    }

    pub async fn list(&self, access_token: &str) -> Result<Vec<Repository>, CommandError> {
        let mut repositories = Vec::new();
        for (backend, endpoint) in self.endpoints() {
            let output = self
                .run(
                    ["repository", "list", endpoint.server_url.as_str()],
                    None,
                    access_token,
                )
                .await?;
            repositories.extend(parse_repositories(
                &output,
                &endpoint.public_server_url,
                backend,
            )?);
        }
        repositories.sort_by_key(|repository| repository.name.to_lowercase());
        for duplicate in repositories.windows(2) {
            if duplicate[0].name.eq_ignore_ascii_case(&duplicate[1].name) {
                return Err(CommandError {
                    message: format!(
                        "repository name '{}' exists in more than one storage backend",
                        duplicate[0].name
                    ),
                });
            }
        }
        Ok(repositories)
    }

    pub async fn storage_backend(
        &self,
        name: &str,
        access_token: &str,
    ) -> Result<StorageBackend, CommandError> {
        self.list(access_token)
            .await?
            .into_iter()
            .find(|repository| repository.name == name)
            .map(|repository| repository.storage_backend)
            .ok_or_else(|| CommandError {
                message: format!("repository '{name}' was not found"),
            })
    }

    pub async fn public_repository_url_for_name(
        &self,
        name: &str,
        access_token: &str,
    ) -> Result<String, CommandError> {
        let backend = self.storage_backend(name, access_token).await?;
        self.public_repository_url_for(backend, name)
    }

    pub async fn create(
        &self,
        name: &str,
        description: Option<&str>,
        storage_backend: StorageBackend,
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
        if self
            .list(access_token)
            .await?
            .iter()
            .any(|repository| repository.name.eq_ignore_ascii_case(name))
        {
            return Err(CommandError {
                message: format!("repository '{name}' already exists"),
            });
        }
        let url = self.public_repository_url_for(storage_backend, name)?;
        let command_url = self.command_repository_url_for(storage_backend, name)?;
        let workspace = TempDir::new().map_err(internal_error)?;
        let mut args = vec!["repository", "create", command_url.as_str()];
        if let Some(description) = description.filter(|value| !value.trim().is_empty()) {
            args.extend(["--description", description.trim()]);
        }
        let output = self.run(args, Some(workspace.path()), access_token).await?;
        parse_created_repository(&output, &url, storage_backend).ok_or_else(|| CommandError {
            message: "Lore did not return the created repository".into(),
        })
    }

    pub async fn delete(&self, name: &str, access_token: &str) -> Result<(), CommandError> {
        self.delete_on(name, StorageBackend::DynamoDbS3, access_token)
            .await
    }

    pub async fn delete_on(
        &self,
        name: &str,
        storage_backend: StorageBackend,
        access_token: &str,
    ) -> Result<(), CommandError> {
        validate_name(name).map_err(|error| CommandError {
            message: error.to_string(),
        })?;
        let url = self.command_repository_url_for(storage_backend, name)?;
        self.run(["repository", "delete", url.as_str()], None, access_token)
            .await?;
        Ok(())
    }

    pub async fn branches(
        &self,
        name: &str,
        access_token: &str,
    ) -> Result<Vec<Branch>, CommandError> {
        self.branches_on(name, StorageBackend::DynamoDbS3, access_token)
            .await
    }

    pub async fn branches_on(
        &self,
        name: &str,
        storage_backend: StorageBackend,
        access_token: &str,
    ) -> Result<Vec<Branch>, CommandError> {
        validate_name(name).map_err(|error| CommandError {
            message: error.to_string(),
        })?;
        let workspace = TempDir::new().map_err(internal_error)?;
        let url = self.command_repository_url_for(storage_backend, name)?;
        self.run(
            [
                "repository",
                "clone",
                "--bare",
                "--",
                url.as_str(),
                "repository",
            ],
            Some(workspace.path()),
            access_token,
        )
        .await?;
        let output = self
            .run(
                ["--repository", "repository", "--remote", "branch", "list"],
                Some(workspace.path()),
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

    pub async fn links_on(
        &self,
        name: &str,
        branch: &str,
        storage_backend: StorageBackend,
        access_token: &str,
    ) -> Result<RepositoryLinks, CommandError> {
        validate_name(name).map_err(|error| CommandError {
            message: error.to_string(),
        })?;
        validate_branch(branch)?;
        let current = self
            .branches_on(name, storage_backend, access_token)
            .await?
            .into_iter()
            .find(|candidate| candidate.name == branch)
            .ok_or_else(|| CommandError {
                message: format!("branch '{branch}' was not found"),
            })?;
        let (_workspace, repository) = self
            .checkout(name, &current.revision, storage_backend, access_token)
            .await?;
        let output = self
            .run(
                ["--repository", ".", "--remote", "link", "list"],
                Some(&repository),
                access_token,
            )
            .await?;
        let mut links = parse_links(&output)?;
        links.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(RepositoryLinks {
            branch: current.name,
            revision: current.revision,
            links,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn add_link_on(
        &self,
        name: &str,
        branch: &str,
        expected_revision: &str,
        path: &str,
        source_url: &str,
        source_path: &str,
        source_repository_id: &str,
        source_branch_id: &str,
        source_revision: &str,
        disable_branching: bool,
        storage_backend: StorageBackend,
        access_token: &str,
    ) -> Result<String, CommandError> {
        validate_link_path(path)?;
        validate_link_source_path(source_path)?;
        validate_revision(source_revision)?;
        self.mutate_link_on(
            name,
            branch,
            expected_revision,
            storage_backend,
            access_token,
            LinkMutation::Add {
                path,
                source_url,
                source_path,
                source_repository_id,
                source_branch_id,
                source_revision,
                disable_branching,
            },
        )
        .await
    }

    pub async fn ensure_source_directory_on(
        &self,
        name: &str,
        branch: &str,
        source_path: &str,
        storage_backend: StorageBackend,
        access_token: &str,
    ) -> Result<bool, CommandError> {
        self.prepare_source_directory_on(
            name,
            branch,
            source_path,
            storage_backend,
            access_token,
            true,
        )
        .await
    }

    pub async fn prepare_source_directory_on(
        &self,
        name: &str,
        branch: &str,
        source_path: &str,
        storage_backend: StorageBackend,
        access_token: &str,
        create_missing: bool,
    ) -> Result<bool, CommandError> {
        validate_name(name).map_err(|error| CommandError {
            message: error.to_string(),
        })?;
        validate_branch(branch)?;
        validate_link_source_path(source_path)?;
        let current = self
            .branches_on(name, storage_backend, access_token)
            .await?
            .into_iter()
            .find(|candidate| candidate.name == branch)
            .ok_or_else(|| CommandError {
                message: format!("source branch '{branch}' was not found"),
            })?;
        if source_path == "." {
            return Ok(false);
        }
        let (_workspace, repository) = self
            .checkout(name, &current.revision, storage_backend, access_token)
            .await?;
        let mut candidate = repository.clone();
        let mut exists = true;
        for component in source_path.split('/') {
            candidate.push(component);
            match tokio::fs::symlink_metadata(&candidate).await {
                Ok(metadata) if metadata.is_dir() => {}
                Ok(_) => {
                    return Err(CommandError {
                        message: format!(
                            "link source path '{source_path}' exists but is not a directory"
                        ),
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    exists = false;
                    break;
                }
                Err(error) => return Err(internal_error(error)),
            }
        }
        if exists {
            return Ok(false);
        }
        if !create_missing {
            return Err(CommandError {
                message: format!(
                    "source folder '{source_path}' does not exist; enable automatic folder creation or create it first"
                ),
            });
        }
        tokio::fs::create_dir_all(repository.join(source_path))
            .await
            .map_err(internal_error)?;
        self.run(
            ["stage", "--scan", "--", source_path],
            Some(&repository),
            access_token,
        )
        .await?;
        let message = format!("Create Lore link source folder {source_path} from LoreHub");
        self.run(
            ["commit", message.as_str()],
            Some(&repository),
            access_token,
        )
        .await?;
        let output = self.run(["push"], Some(&repository), access_token).await?;
        parse_pushed_revision(&output)?;
        Ok(true)
    }

    pub async fn update_link_on(
        &self,
        name: &str,
        branch: &str,
        expected_revision: &str,
        path: &str,
        storage_backend: StorageBackend,
        access_token: &str,
    ) -> Result<String, CommandError> {
        validate_link_path(path)?;
        self.mutate_link_on(
            name,
            branch,
            expected_revision,
            storage_backend,
            access_token,
            LinkMutation::Update { path },
        )
        .await
    }

    pub async fn remove_link_on(
        &self,
        name: &str,
        branch: &str,
        expected_revision: &str,
        path: &str,
        storage_backend: StorageBackend,
        access_token: &str,
    ) -> Result<String, CommandError> {
        validate_link_path(path)?;
        self.mutate_link_on(
            name,
            branch,
            expected_revision,
            storage_backend,
            access_token,
            LinkMutation::Remove { path },
        )
        .await
    }

    async fn mutate_link_on(
        &self,
        name: &str,
        branch: &str,
        expected_revision: &str,
        storage_backend: StorageBackend,
        access_token: &str,
        mutation: LinkMutation<'_>,
    ) -> Result<String, CommandError> {
        validate_name(name).map_err(|error| CommandError {
            message: error.to_string(),
        })?;
        validate_branch(branch)?;
        validate_revision(expected_revision)?;
        let current = self
            .branches_on(name, storage_backend, access_token)
            .await?
            .into_iter()
            .find(|candidate| candidate.name == branch)
            .ok_or_else(|| CommandError {
                message: format!("branch '{branch}' was not found"),
            })?;
        if !current.revision.eq_ignore_ascii_case(expected_revision) {
            return Err(CommandError {
                message: format!(
                    "branch '{branch}' has changed from revision {expected_revision}; reload before saving"
                ),
            });
        }
        let (_workspace, repository) = self
            .checkout(name, expected_revision, storage_backend, access_token)
            .await?;
        let is_update = matches!(&mutation, LinkMutation::Update { .. });
        let (args, message) = match mutation {
            LinkMutation::Add {
                path,
                source_url,
                source_path,
                source_repository_id,
                source_branch_id,
                source_revision,
                disable_branching,
            } => {
                let output = self
                    .run(
                        ["--repository", ".", "--remote", "link", "list"],
                        Some(&repository),
                        access_token,
                    )
                    .await?;
                let existing_links = parse_links(&output)?;
                if let Some(parent) = existing_links
                    .iter()
                    .find(|link| link_path_is_below(path, &link.path))
                {
                    return Err(CommandError {
                        message: format!(
                            "link path '{path}' is inside existing link '{}'",
                            parent.path
                        ),
                    });
                }
                let mut pin = source_revision.to_owned();
                for link in existing_links {
                    if !repository_ids_equal(&link.source_repository_id, source_repository_id)
                        || link.source_branch_id != source_branch_id
                        || link.source_revision.eq_ignore_ascii_case(&pin)
                    {
                        continue;
                    }
                    let output = self
                        .run(
                            ["link", "update", "--", link.path.as_str()],
                            Some(&repository),
                            access_token,
                        )
                        .await?;
                    if let Some(revision) = link_change_revision(&output) {
                        pin = revision;
                        let message = format!(
                            "Update Lore link {} before adding {path} from LoreHub",
                            link.path
                        );
                        self.run(
                            ["commit", message.as_str()],
                            Some(&repository),
                            access_token,
                        )
                        .await?;
                    }
                }
                let mut args = vec![
                    OsString::from("link"),
                    OsString::from("add"),
                    OsString::from("--pin"),
                    OsString::from(pin),
                ];
                if disable_branching {
                    args.push(OsString::from("--disable-branching"));
                }
                args.extend([
                    OsString::from("--"),
                    OsString::from(path),
                    OsString::from(source_url),
                    OsString::from(source_path),
                ]);
                (args, format!("Add Lore link {path} from LoreHub"))
            }
            LinkMutation::Update { path } => (
                vec![
                    OsString::from("link"),
                    OsString::from("update"),
                    OsString::from("--"),
                    OsString::from(path),
                ],
                format!("Update Lore link {path} from LoreHub"),
            ),
            LinkMutation::Remove { path } => (
                vec![
                    OsString::from("link"),
                    OsString::from("remove"),
                    OsString::from("--"),
                    OsString::from(path),
                ],
                format!("Remove Lore link {path} from LoreHub"),
            ),
        };
        let output = self.run(args, Some(&repository), access_token).await?;
        if is_update {
            let changed = link_change_revision(&output)
                .is_some_and(|revision| !revision.bytes().all(|byte| byte == b'0'));
            if !changed {
                return Ok(current.revision);
            }
        }
        self.run(
            ["commit", message.as_str()],
            Some(&repository),
            access_token,
        )
        .await?;
        let output = self.run(["push"], Some(&repository), access_token).await?;
        parse_pushed_revision(&output)
    }

    async fn checkout(
        &self,
        name: &str,
        revision: &str,
        storage_backend: StorageBackend,
        access_token: &str,
    ) -> Result<(TempDir, PathBuf), CommandError> {
        validate_revision(revision)?;
        let workspace = TempDir::new().map_err(internal_error)?;
        let repository = workspace.path().join("repository");
        let url = self.command_repository_url_for(storage_backend, name)?;
        self.run(
            [
                "clone",
                "--revision",
                revision,
                "--",
                url.as_str(),
                "repository",
            ],
            Some(workspace.path()),
            access_token,
        )
        .await?;
        Ok((workspace, repository))
    }

    /// Reads the CI file through Lore so selection always reflects the immutable revision.
    pub async fn pipeline_file(
        &self,
        name: &str,
        revision: &str,
        access_token: &str,
    ) -> Result<Option<PipelineFile>, CommandError> {
        self.pipeline_file_on(name, revision, StorageBackend::DynamoDbS3, access_token)
            .await
    }

    pub async fn pipeline_file_on(
        &self,
        name: &str,
        revision: &str,
        storage_backend: StorageBackend,
        access_token: &str,
    ) -> Result<Option<PipelineFile>, CommandError> {
        self.pipeline_source_on(name, revision, storage_backend, access_token)
            .await?
            .map(|source| {
                PipelineFile::parse(&source).map_err(|error| CommandError {
                    message: format!("invalid {PIPELINE_FILE_NAME}: {error}"),
                })
            })
            .transpose()
    }

    /// Reads the raw CI configuration through Lore at an immutable revision.
    pub async fn pipeline_source_on(
        &self,
        name: &str,
        revision: &str,
        storage_backend: StorageBackend,
        access_token: &str,
    ) -> Result<Option<String>, CommandError> {
        validate_name(name).map_err(|error| CommandError {
            message: error.to_string(),
        })?;
        validate_revision(revision)?;
        let workspace = TempDir::new().map_err(internal_error)?;
        let url = self.command_repository_url_for(storage_backend, name)?;
        self.run(
            ["clone", "--", url.as_str(), "repository"],
            Some(workspace.path()),
            access_token,
        )
        .await?;
        let path = workspace.path().join(PIPELINE_FILE_NAME);
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
                ".",
                "--remote",
                "file",
                "write",
                "--path",
                PIPELINE_FILE_NAME,
                "--revision",
                revision,
                "--output",
            ])
            .arg(&path)
            // Lore resolves --path from the working directory, even when
            // --repository is supplied. Read inside the checkout and write
            // outside it so the requested revision cannot overwrite its files.
            .current_dir(workspace.path().join("repository"))
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
        let error_message = lore_error_message(&stdout)
            .or_else(|| first_line(&String::from_utf8_lossy(&output.stderr)));
        if completion_status == Some(3)
            || ((!output.status.success() || completion_status.is_some_and(|status| status != 0))
                && error_message.as_deref().is_some_and(|message| {
                    message.eq_ignore_ascii_case("file not found: .lore-ci.toml")
                }))
        {
            return Ok(None);
        }
        if !output.status.success() || completion_status != Some(0) {
            return Err(CommandError {
                message: error_message
                    .unwrap_or_else(|| format!("unable to read {PIPELINE_FILE_NAME}")),
            });
        }
        let mut source = Vec::new();
        tokio::fs::File::open(path)
            .await
            .map_err(internal_error)?
            .take(PIPELINE_FILE_MAX_BYTES as u64 + 1)
            .read_to_end(&mut source)
            .await
            .map_err(internal_error)?;
        if source.len() > PIPELINE_FILE_MAX_BYTES {
            return Err(CommandError {
                message: format!("{PIPELINE_FILE_NAME} exceeds 256 KiB"),
            });
        }
        String::from_utf8(source)
            .map(Some)
            .map_err(|_| CommandError {
                message: format!("{PIPELINE_FILE_NAME} must be UTF-8 text"),
            })
    }

    /// Replaces the CI configuration on a branch and publishes a new Lore revision.
    /// The expected revision prevents a browser session from overwriting a newer edit.
    pub async fn update_pipeline_source_on(
        &self,
        name: &str,
        branch: &str,
        expected_revision: &str,
        source: &str,
        storage_backend: StorageBackend,
        access_token: &str,
    ) -> Result<String, CommandError> {
        validate_name(name).map_err(|error| CommandError {
            message: error.to_string(),
        })?;
        validate_branch(branch)?;
        validate_revision(expected_revision)?;
        PipelineFile::parse(source).map_err(|error| CommandError {
            message: format!("invalid {PIPELINE_FILE_NAME}: {error}"),
        })?;

        let branches = self
            .branches_on(name, storage_backend, access_token)
            .await?;
        let current = branches
            .iter()
            .find(|candidate| candidate.name == branch)
            .ok_or_else(|| CommandError {
                message: format!("branch '{branch}' was not found"),
            })?;
        if !current.revision.eq_ignore_ascii_case(expected_revision) {
            return Err(CommandError {
                message: format!(
                    "branch '{branch}' has changed from revision {expected_revision}; reload before saving"
                ),
            });
        }

        let workspace = TempDir::new().map_err(internal_error)?;
        let url = self.command_repository_url_for(storage_backend, name)?;
        self.run(
            [
                "clone",
                "--revision",
                expected_revision,
                "--",
                url.as_str(),
                "repository",
            ],
            Some(workspace.path()),
            access_token,
        )
        .await?;
        let repository = workspace.path().join("repository");
        let path = repository.join(PIPELINE_FILE_NAME);
        match tokio::fs::symlink_metadata(&path).await {
            Ok(metadata) if !metadata.file_type().is_file() => {
                return Err(CommandError {
                    message: format!("{PIPELINE_FILE_NAME} must be a regular file"),
                });
            }
            Ok(_) => {
                if tokio::fs::read(&path).await.map_err(internal_error)? == source.as_bytes() {
                    return Ok(current.revision.clone());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(internal_error(error)),
        }
        tokio::fs::write(&path, source)
            .await
            .map_err(internal_error)?;
        self.run(
            ["stage", PIPELINE_FILE_NAME],
            Some(&repository),
            access_token,
        )
        .await?;
        self.run(
            ["commit", "Update .lore-ci.toml from LoreHub"],
            Some(&repository),
            access_token,
        )
        .await?;
        let output = self.run(["push"], Some(&repository), access_token).await?;
        parse_pushed_revision(&output)
    }

    fn endpoint(&self, backend: StorageBackend) -> Result<&RepositoryEndpoint, CommandError> {
        match backend {
            StorageBackend::DynamoDbS3 => Ok(&self.dynamodb_s3),
            StorageBackend::LocalFile => self.local_file.as_ref().ok_or_else(|| CommandError {
                message: "Local File storage backend is not configured".into(),
            }),
        }
    }

    fn endpoints(&self) -> impl Iterator<Item = (StorageBackend, &RepositoryEndpoint)> {
        std::iter::once((StorageBackend::DynamoDbS3, &self.dynamodb_s3)).chain(
            self.local_file
                .iter()
                .map(|endpoint| (StorageBackend::LocalFile, endpoint)),
        )
    }

    async fn run<I, S>(
        &self,
        args: I,
        working_directory: Option<&Path>,
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
        if let Some(working_directory) = working_directory {
            command.current_dir(working_directory);
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

fn validate_branch(branch: &str) -> Result<(), CommandError> {
    if branch.is_empty() || branch.len() > 255 || branch.chars().any(char::is_control) {
        return Err(CommandError {
            message: "branch must be 1 to 255 characters without control characters".into(),
        });
    }
    Ok(())
}

fn validate_revision(revision: &str) -> Result<(), CommandError> {
    if revision.len() != 64 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CommandError {
            message: "revision must be a full 64-character Lore revision hash".into(),
        });
    }
    Ok(())
}

fn validate_link_path(path: &str) -> Result<(), CommandError> {
    if path.len() > 4096 || !valid_relative_path(path) {
        return Err(CommandError {
            message: "link path must be a safe repository-relative path".into(),
        });
    }
    Ok(())
}

fn validate_link_source_path(path: &str) -> Result<(), CommandError> {
    if path.len() > 4096 || (path != "." && !valid_relative_path(path)) {
        return Err(CommandError {
            message: "link source path must be '.' or a safe repository-relative path".into(),
        });
    }
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

fn parse_repositories(
    output: &str,
    server_url: &str,
    storage_backend: StorageBackend,
) -> Result<Vec<Repository>, CommandError> {
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
            storage_backend,
        });
    }
    Ok(result)
}

fn parse_created_repository(
    output: &str,
    url: &str,
    storage_backend: StorageBackend,
) -> Option<Repository> {
    json_events(output).ok()?.into_iter().find_map(|event| {
        if event.get("tagName")?.as_str()? != "repositoryCreate" {
            return None;
        }
        let data = &event["data"];
        Some(Repository {
            id: value_string(&data["id"])?,
            name: data["name"].as_str()?.to_owned(),
            url: url.to_owned(),
            storage_backend,
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

fn parse_links(output: &str) -> Result<Vec<RepositoryLink>, CommandError> {
    let mut result = Vec::new();
    for event in json_events(output)? {
        if event.get("tagName").and_then(Value::as_str) != Some("linkEntry") {
            continue;
        }
        let data = &event["data"];
        let path = data["linkPath"]
            .as_str()
            .filter(|path| path.len() <= 4096 && valid_relative_path(path))
            .ok_or_else(parse_error)?
            .to_owned();
        let source_repository_id = data["link"]
            .as_str()
            .filter(|value| {
                let value = value.strip_prefix("urc-").unwrap_or(value);
                value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
            .ok_or_else(parse_error)?;
        let source_repository_id = source_repository_id
            .strip_prefix("urc-")
            .unwrap_or(source_repository_id)
            .to_ascii_lowercase();
        let source_path = data["sourcePath"]
            .as_str()
            .ok_or_else(parse_error)?
            .to_owned();
        let source_branch_id = data["branch"]
            .as_str()
            .filter(|value| !value.is_empty() && value.len() <= 256)
            .ok_or_else(parse_error)?
            .to_owned();
        let source_revision = data["revision"]
            .as_str()
            .filter(|revision| {
                revision.len() == 64 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
            .ok_or_else(parse_error)?
            .to_ascii_lowercase();
        let tracking = data["tracking"].as_bool().ok_or_else(parse_error)?;
        result.push(RepositoryLink {
            path,
            source_repository_id,
            source_path,
            source_branch_id,
            source_revision,
            tracking,
        });
    }
    Ok(direct_repository_links(result, |link| &link.path))
}

fn repository_ids_equal(left: &str, right: &str) -> bool {
    left.strip_prefix("urc-")
        .unwrap_or(left)
        .eq_ignore_ascii_case(right.strip_prefix("urc-").unwrap_or(right))
}

fn link_change_revision(output: &str) -> Option<String> {
    json_events(output)
        .ok()?
        .into_iter()
        .rev()
        .find_map(|event| {
            (event["tagName"] == "linkChange")
                .then(|| event["data"]["revision"].as_str())
                .flatten()
                .filter(|revision| {
                    revision.len() == 64 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
                .map(str::to_ascii_lowercase)
        })
}

fn parse_pushed_revision(output: &str) -> Result<String, CommandError> {
    json_events(output)?
        .into_iter()
        .find_map(|event| {
            (event["tagName"] == "branchPushRevisionPushEnd")
                .then(|| event["data"]["newRemoteRevision"].as_str())
                .flatten()
                .filter(|revision| {
                    revision.len() == 64 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
                .map(str::to_ascii_lowercase)
        })
        .ok_or_else(parse_error)
}

fn json_events(output: &str) -> Result<Vec<Value>, CommandError> {
    output
        .lines()
        .filter(|line| line.trim_start().starts_with('{'))
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
    let events = output
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect::<Vec<_>>();
    events
        .iter()
        .rev()
        .find(|event| event.get("tagName").and_then(Value::as_str) == Some("complete"))
        .and_then(|event| event["data"]["error"]["message"].as_str())
        .and_then(first_line)
        .or_else(|| {
            events.iter().rev().find_map(|event| {
                (event.get("tagName")?.as_str()? == "log")
                    .then(|| event["data"]["message"].as_str())
                    .flatten()
                    .and_then(first_line)
            })
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
    async fn creates_repository_on_selected_storage_backend() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let binary = root.path().join("lore");
        std::fs::write(
            &binary,
            r#"#!/bin/sh
set -eu
case "$4:$5" in
  repository:list)
    printf '%s\n' '{"tagName":"complete","data":{"status":0}}'
    ;;
  repository:create)
    [ "$6" = lores://local.internal:41337/project ]
    printf '%s\n' '{"tagName":"repositoryCreate","data":{"id":"local-id","name":"project"}}'
    printf '%s\n' '{"tagName":"complete","data":{"status":0}}'
    ;;
  *) exit 99 ;;
esac
"#,
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let service = RepositoryService::new(
            binary,
            "lores://aws.internal:41337",
            "lores://server.test:41337",
        )
        .unwrap()
        .with_local_backend("lores://local.internal:41337", "lores://server.test:41338")
        .unwrap();

        let repository = service
            .create("project", None, StorageBackend::LocalFile, "")
            .await
            .unwrap();

        assert_eq!(repository.storage_backend, StorageBackend::LocalFile);
        assert_eq!(repository.url, "lores://server.test:41338/project");
    }

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
    # Simulate the current branch config materialized during clone.
    printf '%s\n' 'invalid current branch config' > "${11}/.lore-ci.toml"
    ;;
  --repository)
    [ "$9" = . ]
    [ "$(basename "$PWD")" = repository ]
    [ -f .lore-ci.toml ]
    [ "${10}" = --remote ]
    [ "${11} ${12}" = 'file write' ]
    [ "${13}" = --path ]
    [ "${14}" = .lore-ci.toml ]
    [ "${15}" = --revision ]
    [ "${17}" = --output ]
    case "${16}" in
      b*) printf '%s\n' '{"tagName":"complete","data":{"status":1,"error":{"message":"file not found: .lore-ci.toml"}}}'; exit 1 ;;
      c*) printf '%s\n' '{"tagName":"complete","data":{"status":1,"error":{"message":"permission denied"}}}'; exit 1 ;;
      d*) printf '%s\n' '{"tagName":"complete","data":{"status":1,"error":{"message":"file not found: another-file"}}}'; exit 1 ;;
      e*) printf '%s\n' 'file not found: .lore-ci.toml' >&2; exit 1 ;;
      f*) printf '%s\n' '{"tagName":"complete","data":{"status":3}}'; exit 0 ;;
    esac
    [ "${16}" = aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ]
    # Like Lore, refuse to overwrite a file created by clone.
    [ ! -e "${18}" ]
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
        for revision in ["b", "e", "f"] {
            assert!(
                service
                    .pipeline_file("project", &revision.repeat(64), "test-token")
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        for revision in ["c", "d"] {
            assert!(
                service
                    .pipeline_file("project", &revision.repeat(64), "test-token")
                    .await
                    .is_err()
            );
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn updates_pipeline_file_with_revision_guard() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let binary = root.path().join("lore");
        let pushed = root.path().join("pushed");
        std::fs::write(
            &binary,
            format!(
                r#"#!/bin/sh
set -eu
[ "$1" = --json ]
[ "$4" = --identity-token ]
[ "$6" = --access-token ]
test_token="$5"
shift 7
case "${{1:-}}:${{2:-}}" in
  repository:clone)
    [ "$3" = --bare ]
    mkdir -p "$6"
    ;;
  --repository:repository)
    revision={old_revision}
    [ ! -e '{pushed}' ] || revision={new_revision}
    printf '{{"tagName":"branchListEntry","data":{{"id":"branch-id","name":"main","location":"remote","archived":false,"latest":"%s"}}}}\n' "$revision"
    ;;
  clone:--revision)
    [ "$3" = {old_revision} ]
    mkdir -p "$6"
    if [ "$test_token" != missing-token ]; then
      printf '%s\n' 'stages = ["old"]' > "$6/.lore-ci.toml"
    fi
    ;;
  stage:.lore-ci.toml)
    [ -f .lore-ci.toml ]
    ;;
  commit:*)
    [ "$2" = 'Update .lore-ci.toml from LoreHub' ]
    ;;
  push:)
    cp .lore-ci.toml '{pushed}.content'
    touch '{pushed}'
    printf '%s\n' '{{"tagName":"branchPushRevisionPushEnd","data":{{"newRemoteRevision":"{new_revision}"}}}}'
    ;;
  *) exit 99 ;;
esac
printf '%s\n' '{{"tagName":"complete","data":{{"status":0}}}}'
"#,
                pushed = pushed.display(),
                old_revision = "a".repeat(64),
                new_revision = "b".repeat(64),
            ),
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let service =
            RepositoryService::new(binary, "lores://server.test", "lores://server.test").unwrap();
        let source =
            "stages = ['test']\n[[jobs]]\nname = 'check'\nstage = 'test'\nscript = ['true']\n";

        let revision = service
            .update_pipeline_source_on(
                "project",
                "main",
                &"a".repeat(64),
                source,
                StorageBackend::DynamoDbS3,
                "test-token",
            )
            .await
            .unwrap();

        assert_eq!(revision, "b".repeat(64));
        assert_eq!(
            std::fs::read_to_string(format!("{}.content", pushed.display())).unwrap(),
            source
        );
        let error = service
            .update_pipeline_source_on(
                "project",
                "main",
                &"a".repeat(64),
                source,
                StorageBackend::DynamoDbS3,
                "test-token",
            )
            .await
            .unwrap_err();
        assert!(error.message.contains("has changed"));
        // The same guarded save can create the root file when none exists.
        std::fs::remove_file(&pushed).unwrap();
        let revision = service
            .update_pipeline_source_on(
                "project",
                "main",
                &"a".repeat(64),
                source,
                StorageBackend::DynamoDbS3,
                "missing-token",
            )
            .await
            .unwrap();
        assert_eq!(revision, "b".repeat(64));
        assert_eq!(
            std::fs::read_to_string(format!("{}.content", pushed.display())).unwrap(),
            source
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn creates_and_pushes_a_missing_link_source_directory() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let binary = root.path().join("lore");
        let pushed = root.path().join("pushed");
        std::fs::write(
            &binary,
            format!(
                r#"#!/bin/sh
set -eu
shift 7
case "${{1:-}}:${{2:-}}" in
  repository:clone)
    mkdir -p "$6"
    ;;
  --repository:repository)
    printf '%s\n' '{{"tagName":"branchListEntry","data":{{"id":"branch-id","name":"main","location":"remote","archived":false,"latest":"{old_revision}"}}}}'
    ;;
  clone:--revision)
    [ "$3" = {old_revision} ]
    mkdir -p "$6"
    ;;
  stage:--scan)
    [ "$3" = -- ]
    [ "$4" = Libraries/Shared ]
    [ -d Libraries/Shared ]
    ;;
  commit:*)
    [ "$2" = 'Create Lore link source folder Libraries/Shared from LoreHub' ]
    ;;
  push:)
    touch '{pushed}'
    printf '%s\n' '{{"tagName":"branchPushRevisionPushEnd","data":{{"newRemoteRevision":"{new_revision}"}}}}'
    ;;
  *) exit 99 ;;
esac
printf '%s\n' '{{"tagName":"complete","data":{{"status":0}}}}'
"#,
                pushed = pushed.display(),
                old_revision = "a".repeat(64),
                new_revision = "b".repeat(64),
            ),
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let service =
            RepositoryService::new(binary, "lores://server.test", "lores://server.test").unwrap();

        let error = service
            .prepare_source_directory_on(
                "source",
                "main",
                "Libraries/Shared",
                StorageBackend::DynamoDbS3,
                "test-token",
                false,
            )
            .await
            .unwrap_err();
        assert!(error.message.contains("does not exist"));
        assert!(
            !pushed.exists(),
            "disabled folder creation must not commit or push"
        );
        assert!(
            service
                .ensure_source_directory_on(
                    "source",
                    "main",
                    "Libraries/Shared",
                    StorageBackend::DynamoDbS3,
                    "test-token",
                )
                .await
                .unwrap()
        );
        assert!(pushed.exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn reconciles_existing_source_links_before_adding_another() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let binary = root.path().join("lore");
        std::fs::write(
            &binary,
            format!(
                r#"#!/bin/sh
set -eu
shift 7
case "${{1:-}}:${{2:-}}" in
  repository:clone)
    mkdir -p "$6"
    ;;
  --repository:repository)
    printf '%s\n' '{{"tagName":"branchListEntry","data":{{"id":"root-branch","name":"main","location":"remote","archived":false,"latest":"{root_revision}"}}}}'
    ;;
  clone:--revision)
    [ "$3" = {root_revision} ]
    mkdir -p "$6"
    ;;
  --repository:.)
    [ "$3" = --remote ]
    [ "$4 $5" = 'link list' ]
    printf '%s\n' '{{"tagName":"linkEntry","data":{{"link":"{source_repository}","linkPath":"Existing","sourcePath":"Existing","branch":"source-branch","tracking":true,"revision":"{old_source_revision}"}}}}'
    ;;
  link:update)
    [ "$3" = -- ]
    [ "$4" = Existing ]
    printf '%s\n' '{{"tagName":"linkChange","data":{{"revision":"{new_source_revision}"}}}}'
    ;;
  link:add)
    [ "$3" = --pin ]
    [ "$4" = {new_source_revision} ]
    [ "$5" = -- ]
    [ "$6" = New ]
    [ "$8" = New ]
    ;;
  commit:*) ;;
  push:)
    printf '%s\n' '{{"tagName":"branchPushRevisionPushEnd","data":{{"newRemoteRevision":"{new_root_revision}"}}}}'
    ;;
  *) exit 99 ;;
esac
printf '%s\n' '{{"tagName":"complete","data":{{"status":0}}}}'
"#,
                root_revision = "a".repeat(64),
                new_root_revision = "b".repeat(64),
                old_source_revision = "c".repeat(64),
                new_source_revision = "d".repeat(64),
                source_repository = "1".repeat(32),
            ),
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let service =
            RepositoryService::new(binary, "lores://server.test", "lores://server.test").unwrap();

        let revision = service
            .add_link_on(
                "root",
                "main",
                &"a".repeat(64),
                "New",
                "lores://server.test/source",
                "New",
                &"1".repeat(32),
                "source-branch",
                &"d".repeat(64),
                false,
                StorageBackend::DynamoDbS3,
                "test-token",
            )
            .await
            .unwrap();

        assert_eq!(revision, "b".repeat(64));
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
        let service = RepositoryService::new(
            "lore",
            "lores://aws.internal:41337",
            "lores://server.test:41337",
        )
        .unwrap()
        .with_local_backend("lores://local.internal:41337", "lores://server.test:41338")
        .unwrap();
        assert_eq!(
            service
                .public_repository_url_for(StorageBackend::LocalFile, "engine")
                .unwrap(),
            "lores://server.test:41338/engine"
        );
        assert_eq!(
            service.storage_backend_for_public_url("engine", "lores://server.test:41338/engine"),
            Some(StorageBackend::LocalFile)
        );
        assert_eq!(
            serde_json::to_string(&service.available_storage_backends()).unwrap(),
            r#"["dynamodb_s3","local_file"]"#
        );
    }

    #[test]
    fn parses_lore_json_events() {
        let output = concat!(
            "{\"tagName\":\"repositoryListEntry\",\"data\":{\"id\":\"abc123\",\"name\":\"engine\"}}\n",
            "{\"tagName\":\"complete\",\"data\":{\"status\":0}}\n",
            "No more repositories found.\n"
        );
        assert_eq!(
            parse_repositories(output, "lores://server:41337", StorageBackend::DynamoDbS3,)
                .unwrap(),
            vec![Repository {
                id: "abc123".into(),
                name: "engine".into(),
                url: "lores://server:41337/engine".into(),
                storage_backend: StorageBackend::DynamoDbS3,
            }]
        );
    }

    #[test]
    fn parses_repository_links_around_human_output() {
        let output = format!(
            concat!(
                "{{\"tagName\":\"linkEntry\",\"data\":{{\"link\":\"urc-0194b726b34e72b0b45550b88a967076\",\"linkPath\":\"Dependencies/Source\",\"sourcePath\":\"Libraries/Core\",\"branch\":\"source-main-branch\",\"tracking\":true,\"revision\":\"{}\"}}}}\n",
                "No more links found.\n"
            ),
            "A".repeat(64),
        );
        assert_eq!(
            parse_links(&output).unwrap(),
            vec![RepositoryLink {
                path: "Dependencies/Source".into(),
                source_repository_id: "0194b726b34e72b0b45550b88a967076".into(),
                source_path: "Libraries/Core".into(),
                source_branch_id: "source-main-branch".into(),
                source_revision: "a".repeat(64),
                tracking: true,
            }]
        );
    }

    #[test]
    fn reports_lore_completion_failure_reason() {
        let output = concat!(
            "{\"tagName\":\"log\",\"data\":{\"message\":\"link operation failed\"}}\n",
            "{\"tagName\":\"complete\",\"data\":{\"status\":1,\"error\":{\"message\":\"Source branch 'missing' was not found\\ntrace details\"}}}\n"
        );
        assert_eq!(
            lore_error_message(output).as_deref(),
            Some("Source branch 'missing' was not found")
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
