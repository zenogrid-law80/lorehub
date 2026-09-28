//! Branch through Lore so tracking links follow the same branch identity.
use super::*;

impl RepositoryService {
    pub async fn create_branch_on(
        &self,
        name: &str,
        branch: &str,
        from_branch: &str,
        expected_revision: &str,
        backend: StorageBackend,
        token: &str,
    ) -> Result<Branch, CommandError> {
        validate_name(name).map_err(|error| CommandError {
            message: error.to_string(),
        })?;
        validate_branch(branch)?;
        validate_branch(from_branch)?;
        validate_revision(expected_revision)?;
        let branches = self.branches_on(name, backend, token).await?;
        if branches.iter().any(|candidate| candidate.name == branch) {
            return Err(CommandError {
                message: format!("branch '{branch}' already exists"),
            });
        }
        let base = branches
            .iter()
            .find(|candidate| candidate.name == from_branch)
            .ok_or_else(|| CommandError {
                message: format!("branch '{from_branch}' was not found"),
            })?;
        let changed = || CommandError {
            message: format!("branch '{from_branch}' has changed; reload before creating a branch"),
        };
        if !base.revision.eq_ignore_ascii_case(expected_revision) {
            return Err(changed());
        }

        let workspace = TempDir::new().map_err(internal_error)?;
        let url = self.command_repository_url_for(backend, name)?;
        // Clone the selected head even if the default branch is unavailable.
        // A revision does not identify its containing branch, so switch the
        // bare clone explicitly before Lore creates linked branches.
        self.run(
            [
                "clone",
                "--bare",
                "--branch",
                &base.id,
                "--",
                &url,
                "repository",
            ],
            Some(workspace.path()),
            token,
        )
        .await?;
        let cwd = workspace.path().join("repository");
        self.run(
            ["branch", "switch", "--bare", "--", &base.id],
            Some(&cwd),
            token,
        )
        .await?;
        let output = self
            .run(["--local", "branch", "list"], Some(&cwd), token)
            .await?;
        let unchanged = json_events(&output)?.iter().any(|event| {
            event["tagName"] == "branchListEntry"
                && event["data"]["location"] == "local"
                && event["data"]["isCurrent"] == true
                && event["data"]["id"] == base.id
                && event["data"]["latest"]
                    .as_str()
                    .is_some_and(|revision| revision.eq_ignore_ascii_case(expected_revision))
        });
        if !unchanged {
            return Err(changed());
        }
        // Lore recursively creates Source branches and updates tracking links;
        // fixed links retain their pins. Never force/overwrite an existing branch.
        let output = self
            .run(["branch", "create", "--", branch], Some(&cwd), token)
            .await?;
        let linked_branches: Vec<(String, String)> = json_events(&output)?
            .into_iter()
            .filter(|event| event["tagName"] == "linkBranchCreate")
            .map(|event| {
                Ok((
                    event["data"]["linkPath"]
                        .as_str()
                        .ok_or_else(parse_error)?
                        .to_owned(),
                    event["data"]["branch"]
                        .as_str()
                        .ok_or_else(parse_error)?
                        .to_owned(),
                ))
            })
            .collect::<Result<_, CommandError>>()?;
        // Branch creation can write a new head containing explicit Source pins
        // while leaving the checkout anchor at the parent revision. An unnamed
        // push uses that anchor and silently publishes the old pins instead.
        self.run(["push", "--", branch], Some(&cwd), token).await?;
        let created = self
            .branches_on(name, backend, token)
            .await?
            .into_iter()
            .find(|candidate| candidate.name == branch)
            .ok_or_else(parse_error)?;
        let links = self.links_on(name, branch, backend, token).await?;
        // Verify explicit pins as well as implicit tracking links against the
        // Source branches Lore actually created or reused.
        for (path, source_branch) in linked_branches {
            if !links
                .links
                .iter()
                .any(|link| link.path == path && link.source_branch_id == source_branch)
            {
                return Err(CommandError {
                    message: format!(
                        "branch '{branch}' was created but link '{path}' does not target its Source branch"
                    ),
                });
            }
        }
        for link in direct_repository_links(links.links, |link| &link.path) {
            if link.tracking && link.source_branch_id != created.id {
                return Err(CommandError {
                    message: format!(
                        "branch '{branch}' was created but link '{}' does not target its Source branch",
                        link.path
                    ),
                });
            }
        }
        Ok(created)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn service(root: &Path) -> RepositoryService {
        let binary = root.join("lore");
        std::fs::write(
            &binary,
            include_str!("../../../tests/fixtures/branch-lore.sh"),
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        RepositoryService::new(
            binary,
            "lores://aws.test:41337",
            "lores://public.test:41337",
        )
        .unwrap()
        .with_local_backend("lores://local.test:41337", "lores://public.test:41338")
        .unwrap()
    }

    #[tokio::test]
    async fn branches_from_selected_identity_and_publishes_without_forcing() {
        for backend in [StorageBackend::DynamoDbS3, StorageBackend::LocalFile] {
            let root = tempfile::tempdir().unwrap();
            let service = service(root.path());
            let branch = service
                .create_branch_on(
                    "root",
                    "feature/release",
                    "release",
                    &"a".repeat(64),
                    backend,
                    "test-token",
                )
                .await
                .unwrap();
            assert_eq!(branch.name, "feature/release");
            assert!(root.path().join("source-created").exists());
            assert!(root.path().join("published").exists());
            let commands = std::fs::read_to_string(root.path().join("commands")).unwrap();
            let host = if backend == StorageBackend::LocalFile {
                "local"
            } else {
                "aws"
            };
            assert!(commands.contains(&format!(
                "clone --bare --branch release-id -- lores://{host}.test:41337/root repository"
            )));
            assert!(commands.contains("branch switch --bare -- release-id"));
            assert!(
                commands.contains("branch create -- feature/release\npush -- feature/release\n")
            );
            assert!(!commands.contains("--force"));
            assert!(!commands.contains("test-token"));
        }
    }

    #[tokio::test]
    async fn publishes_the_new_branch_head_with_explicit_pins_not_the_parent_anchor() {
        let root = tempfile::tempdir().unwrap();
        let service = service(root.path());
        std::fs::write(root.path().join("explicit-pins"), "").unwrap();
        let branch = service
            .create_branch_on(
                "root",
                "feature/release",
                "release",
                &"a".repeat(64),
                StorageBackend::DynamoDbS3,
                "",
            )
            .await
            .unwrap();
        assert_eq!(branch.revision, "b".repeat(64));
        let links = service
            .links_on("root", &branch.name, StorageBackend::DynamoDbS3, "")
            .await
            .unwrap();
        let source = links
            .links
            .iter()
            .find(|link| link.path == "Source")
            .unwrap();
        assert!(!source.tracking);
        assert_eq!(source.source_branch_id, "feature-id");
        assert_eq!(
            links
                .links
                .iter()
                .find(|link| link.path == "Fixed")
                .unwrap()
                .source_branch_id,
            "fixed-id"
        );
    }

    #[tokio::test]
    async fn shared_revision_links_resolve_against_the_selected_root_branch() {
        let root = tempfile::tempdir().unwrap();
        let service = service(root.path());
        std::fs::write(root.path().join("published"), "").unwrap();
        for (branch, source_branch) in
            [("release", "release-id"), ("feature/release", "feature-id")]
        {
            let links = service
                .links_on("root", branch, StorageBackend::DynamoDbS3, "")
                .await
                .unwrap();
            assert_eq!(links.revision, "a".repeat(64));
            assert_eq!(
                links
                    .links
                    .iter()
                    .find(|link| link.path == "Source")
                    .unwrap()
                    .source_branch_id,
                source_branch
            );
            assert_eq!(
                links
                    .links
                    .iter()
                    .find(|link| link.path == "Fixed")
                    .unwrap()
                    .source_branch_id,
                "fixed-id"
            );
        }
    }

    #[tokio::test]
    async fn manual_source_update_commits_to_selected_root_even_when_revisions_are_shared() {
        let root = tempfile::tempdir().unwrap();
        let service = service(root.path());
        for marker in ["published", "source-created"] {
            std::fs::write(root.path().join(marker), "").unwrap();
        }
        let revision = service
            .update_link_on(
                "root",
                "feature/release",
                &"a".repeat(64),
                "Source",
                StorageBackend::DynamoDbS3,
                "",
            )
            .await
            .unwrap();
        assert_eq!(revision, "c".repeat(64));
    }

    #[tokio::test]
    async fn creation_rejects_a_published_branch_with_an_incorrect_source_link() {
        let root = tempfile::tempdir().unwrap();
        let service = service(root.path());
        std::fs::write(root.path().join("wrong-link"), "").unwrap();
        let error = service
            .create_branch_on(
                "root",
                "feature/release",
                "release",
                &"a".repeat(64),
                StorageBackend::DynamoDbS3,
                "",
            )
            .await
            .unwrap_err();
        assert!(error.message.contains("does not target its Source branch"));
        assert!(root.path().join("published").exists());
    }

    #[tokio::test]
    async fn rejects_duplicates_missing_or_stale_bases_before_creating_sources() {
        for (name, base, revision, marker, message) in [
            ("release", "release", "a", "", "already exists"),
            ("feature/release", "missing", "a", "", "not found"),
            ("feature/release", "release", "b", "", "has changed"),
            ("feature/release", "release", "a", "advanced", "has changed"),
        ] {
            let root = tempfile::tempdir().unwrap();
            let service = service(root.path());
            if !marker.is_empty() {
                std::fs::write(root.path().join(marker), "").unwrap();
            }
            let error = service
                .create_branch_on(
                    "root",
                    name,
                    base,
                    &revision.repeat(64),
                    StorageBackend::DynamoDbS3,
                    "",
                )
                .await
                .unwrap_err();
            assert!(error.message.contains(message), "{}", error.message);
            assert!(!root.path().join("source-created").exists());
            assert!(!root.path().join("published").exists());
        }
    }

    #[tokio::test]
    async fn reports_source_and_push_failures_without_claiming_success_or_deleting_sources() {
        for (marker, source_created, message) in [
            ("denied", false, "Source repository permission denied"),
            ("fail-push", true, "Root push failed"),
        ] {
            let root = tempfile::tempdir().unwrap();
            let service = service(root.path());
            std::fs::write(root.path().join(marker), "").unwrap();
            let error = service
                .create_branch_on(
                    "root",
                    "feature/release",
                    "release",
                    &"a".repeat(64),
                    StorageBackend::DynamoDbS3,
                    "",
                )
                .await
                .unwrap_err();
            assert_eq!(error.message, message);
            assert_eq!(root.path().join("source-created").exists(), source_created);
            assert!(!root.path().join("published").exists());
        }
    }
}
