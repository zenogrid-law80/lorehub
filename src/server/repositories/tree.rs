//! Read one directory at an immutable revision without materializing file contents.
use super::*;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct TreeEntry {
    pub name: String,
    pub kind: &'static str,
    pub is_link: bool,
}

impl RepositoryService {
    pub async fn tree_on(
        &self,
        name: &str,
        revision: &str,
        path: &str,
        backend: StorageBackend,
        token: &str,
    ) -> Result<Vec<TreeEntry>, CommandError> {
        validate_name(name).map_err(internal_error)?;
        validate_revision(revision)?;
        validate_tree_path(path)?;
        let repositories = self.list(token).await?;
        let mut name = name.to_owned();
        let mut revision = revision.to_owned();
        let mut path = path.to_owned();
        let mut backend = backend;
        // Each hop follows the link's pinned revision, never its branch head.
        for _ in 0..32 {
            let workspace = TempDir::new().map_err(internal_error)?;
            let url = self.command_repository_url_for(backend, &name)?;
            self.run(
                [
                    "repository",
                    "clone",
                    "--bare",
                    "--revision",
                    &revision,
                    "--",
                    &url,
                    "repository",
                ],
                Some(workspace.path()),
                token,
            )
            .await?;
            let cwd = workspace.path().join("repository");
            let mut link_path = String::new();
            let mut target = None;
            // Inspect only ancestors of the requested directory. Enumerating all
            // links would let an unrelated inaccessible source break browsing.
            for component in path.split('/').filter(|part| !part.is_empty()) {
                if !link_path.is_empty() {
                    link_path.push('/');
                }
                link_path.push_str(component);
                let output = self
                    .run(
                        [
                            "--repository",
                            ".",
                            "--remote",
                            "repository",
                            "dump",
                            "--revision",
                            &revision,
                            &format!("--path={link_path}"),
                            "--max-depth",
                            "1",
                        ],
                        Some(&cwd),
                        token,
                    )
                    .await?;
                let events = json_events(&output)?;
                let node = events
                    .iter()
                    .find(|event| event["tagName"] == "repositoryStateDumpNode")
                    .ok_or_else(|| CommandError {
                        message: "directory was not found".into(),
                    })?;
                let flags = node["data"]["flags"].as_u64().ok_or_else(parse_error)?;
                if flags & 2 != 0 {
                    let output = self
                        .run(
                            [
                                "--repository",
                                ".",
                                "--remote",
                                "link",
                                "info",
                                "--",
                                &link_path,
                            ],
                            Some(&cwd),
                            token,
                        )
                        .await?;
                    let link = parse_tree_link(&output)?;
                    if link.path != link_path {
                        return Err(parse_error());
                    }
                    target = Some(link);
                    break;
                }
                if flags & 1 != 0 {
                    return Err(CommandError {
                        message: "requested path is not a directory".into(),
                    });
                }
            }
            if let Some(link) = target {
                let source = repositories
                    .iter()
                    .find(|repo| repository_ids_equal(&repo.id, &link.source_repository_id))
                    .ok_or_else(|| CommandError {
                        message: "linked repository is unavailable or access is denied".into(),
                    })?;
                path = linked_tree_path(&link, &path)?;
                name = source.name.clone();
                backend = source.storage_backend;
                revision = link.source_revision;
                continue;
            }
            let mut args = vec![
                "--repository",
                ".",
                "--remote",
                "repository",
                "dump",
                "--revision",
                &revision,
                "--max-depth",
            ];
            args.push(if path.is_empty() { "1" } else { "2" });
            let path_arg = format!("--path={path}");
            if !path.is_empty() {
                args.push(&path_arg);
            }
            let output = self.run(args, Some(&cwd), token).await?;
            return parse_tree(&output, &path);
        }
        Err(CommandError {
            message: "repository link nesting exceeds 32 levels".into(),
        })
    }
}

pub(super) fn validate_tree_path(path: &str) -> Result<(), CommandError> {
    if path.len() > 4096
        || path.split('/').count() > 128
        || (!path.is_empty() && !valid_relative_path(path))
    {
        return Err(CommandError {
            message: "path must be empty or a safe repository-relative directory".into(),
        });
    }
    Ok(())
}

fn parse_tree_link(output: &str) -> Result<RepositoryLink, CommandError> {
    let entry = json_events(output)?
        .into_iter()
        .find(|event| event["tagName"] == "linkInfo")
        .ok_or_else(parse_error)?;
    let event = serde_json::json!({"tagName": "linkEntry", "data": entry["data"]["entry"]});
    parse_links(&event.to_string())?
        .into_iter()
        .next()
        .ok_or_else(parse_error)
}

fn linked_tree_path(link: &RepositoryLink, path: &str) -> Result<String, CommandError> {
    validate_link_source_path(&link.source_path)?;
    let suffix = path
        .strip_prefix(&link.path)
        .ok_or_else(parse_error)?
        .trim_start_matches('/');
    let source = if link.source_path == "." {
        ""
    } else {
        &link.source_path
    };
    let result = [source, suffix]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    validate_tree_path(&result)?;
    Ok(result)
}

fn parse_tree(output: &str, path: &str) -> Result<Vec<TreeEntry>, CommandError> {
    let events = json_events(output)?;
    if !events
        .iter()
        .any(|event| event["tagName"] == "repositoryStateDump")
    {
        return Err(parse_error());
    }
    let mut entries = Vec::new();
    let prefix = path.rsplit('/').next().unwrap_or("");
    let mut found_directory = path.is_empty();
    for event in events
        .iter()
        .filter(|event| event["tagName"] == "repositoryStateDumpNode")
    {
        let data = &event["data"];
        let raw_name = data["name"].as_str().ok_or_else(parse_error)?;
        let flags = data["flags"].as_u64().ok_or_else(parse_error)?;
        let is_link = flags & 2 != 0;
        let is_directory = flags & 3 == 0;
        let relative = raw_name.trim_end_matches('/');
        let name = if path.is_empty() {
            relative
        } else {
            if relative == prefix {
                if !is_directory {
                    return Err(CommandError {
                        message: "requested path is not a directory".into(),
                    });
                }
                found_directory = true;
                continue;
            }
            let Some(name) = relative
                .strip_prefix(prefix)
                .and_then(|name| name.strip_prefix('/'))
            else {
                continue;
            };
            name
        };
        if name.contains('/') {
            continue;
        }
        if name.is_empty() || !valid_relative_path(name) {
            return Err(parse_error());
        }
        entries.push(TreeEntry {
            name: name.to_owned(),
            kind: if is_directory || is_link {
                "directory"
            } else {
                "file"
            },
            is_link,
        });
    }
    if !found_directory {
        return Err(CommandError {
            message: "directory was not found".into(),
        });
    }
    entries.sort_by(|a, b| {
        (a.kind != "directory", a.name.to_lowercase(), &a.name).cmp(&(
            b.kind != "directory",
            b.name.to_lowercase(),
            &b.name,
        ))
    });
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dump(nodes: &[(&str, u64)]) -> String {
        std::iter::once(serde_json::json!({"tagName":"repositoryStateDump", "data":{}}).to_string()).chain(nodes.iter().map(|(name, flags)| serde_json::json!({"tagName":"repositoryStateDumpNode", "data":{"name":name,"flags":flags}}).to_string())).collect::<Vec<_>>().join("\n")
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn tree_reads_bare_snapshots_and_follows_pinned_links() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("lore");
        let root_revision = "a".repeat(64);
        let source_revision = "b".repeat(64);
        let script = r#"#!/bin/sh
set -eu
shift 7
case "$1:$2" in
  repository:list)
    printf '%s\n' '{"tagName":"repositoryListEntry","data":{"id":"11111111111111111111111111111111","name":"root"}}' '{"tagName":"repositoryListEntry","data":{"id":"22222222222222222222222222222222","name":"source"}}'
    ;;
  repository:clone)
    [ "$3" = '--bare' ] && [ "$4" = '--revision' ] && [ "$6" = '--' ]
    case "$7" in
      */root) [ "$5" = 'ROOT_REVISION' ] ;;
      */source) [ "$5" = 'SOURCE_REVISION' ] ;;
      *) exit 9 ;;
    esac
    mkdir repository
    printf '%s' "$7" > repository/origin
    ;;
  --repository:.)
    shift 3
    case "$1:$2" in
      link:info)
        [ "$3" = '--' ] && [ "$4" = 'Shared' ]
        printf '%s\n' '{"tagName":"linkInfo","data":{"entry":{"linkPath":"Shared","link":"22222222222222222222222222222222","sourcePath":"Library","branch":"source-main","revision":"SOURCE_REVISION","tracking":true}}}'
        ;;
      repository:dump)
        [ "$3" = '--revision' ]
        if [ "$5" != '--max-depth' ]; then
          [ "$6" = '--max-depth' ] && [ "$7" = '1' ]
          case "${5#--path=}" in
            Shared) [ "$4" = 'ROOT_REVISION' ]; node='{"name":"Shared","flags":2}' ;;
            Library) [ "$4" = 'SOURCE_REVISION' ]; node='{"name":"Library/","flags":0}' ;;
            Library/Textures) [ "$4" = 'SOURCE_REVISION' ]; node='{"name":"Textures/","flags":0}' ;;
            *) exit 12 ;;
          esac
          printf '%s\n' "{\"tagName\":\"repositoryStateDumpNode\",\"data\":$node}"
        else
          [ "$4" = 'SOURCE_REVISION' ] && [ "$5" = '--max-depth' ] && [ "$6" = '2' ] && [ "$7" = '--path=Library/Textures' ]
          printf '%s\n' '{"tagName":"repositoryStateDump","data":{}}' '{"tagName":"repositoryStateDumpNode","data":{"name":"Textures/","flags":0}}' '{"tagName":"repositoryStateDumpNode","data":{"name":"Textures/picture.png","flags":1}}'
        fi
        ;;
      *) exit 10 ;;
    esac
    ;;
  *) exit 11 ;;
esac
"#.replace("ROOT_REVISION", &root_revision).replace("SOURCE_REVISION", &source_revision);
        std::fs::write(&binary, script).unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let service =
            RepositoryService::new(binary.as_os_str(), "lores://test", "lores://test").unwrap();
        let entries = service
            .tree_on(
                "root",
                &root_revision,
                "Shared/Textures",
                StorageBackend::DynamoDbS3,
                "fixture-token",
            )
            .await
            .unwrap();
        assert_eq!(
            entries,
            vec![TreeEntry {
                name: "picture.png".into(),
                kind: "file",
                is_link: false
            }]
        );
    }

    #[test]
    fn tree_lists_direct_children_and_distinguishes_links() {
        let entries = parse_tree(&dump(&[("readme", 1), ("src/", 0), ("Shared", 2)]), "").unwrap();
        assert_eq!(
            entries
                .iter()
                .map(|e| (e.name.as_str(), e.kind, e.is_link))
                .collect::<Vec<_>>(),
            vec![
                ("Shared", "directory", true),
                ("src", "directory", false),
                ("readme", "file", false)
            ]
        );
        let entries = parse_tree(
            &dump(&[
                ("src/", 0),
                ("src/deep/", 0),
                ("src/deep/hidden", 1),
                ("src/main.rs", 1),
            ]),
            "client/src",
        )
        .unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "deep");
        assert!(
            parse_tree(&dump(&[("empty/", 0)]), "empty")
                .unwrap()
                .is_empty()
        );
        assert!(parse_tree(&dump(&[("file", 1)]), "file").is_err());
        assert!(parse_tree(&dump(&[]), "missing").is_err());
    }
    #[test]
    fn linked_paths_preserve_the_source_directory_and_reject_escapes() {
        let mut link = RepositoryLink {
            path: "Shared".into(),
            source_repository_id: String::new(),
            source_path: "Libraries/Common".into(),
            source_branch_id: String::new(),
            source_revision: String::new(),
            tracking: true,
        };
        assert_eq!(
            linked_tree_path(&link, "Shared/Textures").unwrap(),
            "Libraries/Common/Textures"
        );
        link.source_path = ".".into();
        assert_eq!(linked_tree_path(&link, "Shared").unwrap(), "");
        for path in ["../secret", "/etc", "foo/../bar", "foo\\bar"] {
            assert!(validate_tree_path(path).is_err());
        }
    }
}
