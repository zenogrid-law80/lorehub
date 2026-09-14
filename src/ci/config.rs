use std::collections::HashSet;

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SubmitPipeline {
    pub repository_url: String,
    pub revision: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub pipeline_name: Option<String>,
}

impl SubmitPipeline {
    pub fn validate(&self) -> Result<()> {
        let url = url::Url::parse(&self.repository_url)?;
        ensure!(url.scheme() == "lores", "repository_url must use lores://");
        ensure!(
            url.host_str().is_some() && url.path().len() > 1,
            "repository URL requires a host and repository path"
        );
        ensure!(
            url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none(),
            "credentials, queries and fragments are not allowed in repository URLs"
        );
        ensure!(
            self.repository_url.len() <= 2048,
            "repository URL is too long"
        );
        ensure!(
            self.revision.len() == 64 && self.revision.bytes().all(|b| b.is_ascii_hexdigit()),
            "revision must be a full 64-character Lore revision hash"
        );
        if let Some(branch) = self.branch.as_deref() {
            ensure!(
                (1..=255).contains(&branch.len()) && !branch.chars().any(char::is_control),
                "branch must be 1 to 255 characters without control characters"
            );
        }
        if let Some(name) = self.pipeline_name.as_deref() {
            ensure!(
                (1..=100).contains(&name.len()) && !name.chars().any(char::is_control),
                "pipeline_name must be 1 to 100 characters without control characters"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct PipelineConfig {
    pub stages: Vec<String>,
    pub jobs: Vec<JobConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct JobConfig {
    pub name: String,
    pub stage: String,
    pub script: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

/// A repository can keep the original single pipeline, or opt into push CI.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineFile {
    #[serde(default)]
    stages: Vec<String>,
    #[serde(default)]
    jobs: Vec<JobConfig>,
    #[serde(default)]
    pub pipelines: Vec<NamedPipeline>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedPipeline {
    pub name: String,
    #[serde(default = "default_category")]
    pub category: String,
    pub runner_os: String,
    #[serde(default)]
    pub sparse_view: Option<String>,
    pub changes: Vec<String>,
    pub working_directory: String,
    pub stages: Vec<String>,
    pub jobs: Vec<JobConfig>,
}

impl NamedPipeline {
    pub fn config(&self) -> PipelineConfig {
        PipelineConfig {
            stages: self.stages.clone(),
            jobs: self.jobs.clone(),
        }
    }

    pub fn matches(&self, paths: &[String]) -> bool {
        self.changes.iter().any(|pattern| {
            paths.iter().any(|path| {
                if let Some(directory) = pattern.strip_suffix("/**") {
                    path == directory
                        || path
                            .strip_prefix(directory)
                            .is_some_and(|tail| tail.starts_with('/'))
                } else {
                    path == pattern
                }
            })
        })
    }
}

pub fn valid_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':', '\0'])
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

impl PipelineFile {
    pub fn parse(source: &str) -> Result<Self> {
        ensure!(source.len() <= 256 * 1024, ".lore-ci.toml exceeds 256 KiB");
        let mut file: Self = toml::from_str(source)?;
        if file.pipelines.is_empty() {
            let config = PipelineConfig::parse(source)?;
            file.stages = config.stages;
            file.jobs = config.jobs;
        } else {
            ensure!(
                file.stages.is_empty() && file.jobs.is_empty(),
                "cannot mix root jobs with named pipelines"
            );
            ensure!(file.pipelines.len() <= 32, "provide at most 32 pipelines");
            let mut names = HashSet::new();
            for pipeline in &mut file.pipelines {
                ensure!(
                    valid_name(&pipeline.name) && names.insert(pipeline.name.clone()),
                    "invalid or duplicate pipeline name"
                );
                ensure!(valid_name(&pipeline.category), "invalid pipeline category");
                ensure!(
                    matches!(pipeline.runner_os.as_str(), "windows" | "macos" | "linux"),
                    "runner_os must be windows, macos or linux"
                );
                if let Some(view) = pipeline.sparse_view.as_mut() {
                    *view = view.trim().to_owned();
                    ensure!(
                        !view.is_empty()
                            && view.len() <= 100
                            && !view.chars().any(char::is_control),
                        "sparse_view must be 1 to 100 characters without control characters"
                    );
                }
                ensure!(
                    pipeline.working_directory == "."
                        || valid_relative_path(&pipeline.working_directory),
                    "working_directory must stay inside the checkout"
                );
                ensure!(
                    !pipeline.changes.is_empty() && pipeline.changes.len() <= 128,
                    "provide 1..128 change paths"
                );
                for pattern in &pipeline.changes {
                    let path = pattern.strip_suffix("/**").unwrap_or(pattern);
                    ensure!(
                        valid_relative_path(path) && !path.contains(['*', '?', '[', ']']),
                        "changes must contain relative exact paths or directory/** patterns"
                    );
                }
                let mut config = pipeline.config();
                config.validate()?;
                pipeline.jobs = config.jobs;
            }
        }
        Ok(file)
    }

    pub fn select(
        &self,
        name: Option<&str>,
    ) -> Result<(PipelineConfig, &str, Option<&str>, Option<&str>)> {
        match name {
            Some(name) => {
                let pipeline = self
                    .pipelines
                    .iter()
                    .find(|p| p.name == name)
                    .ok_or_else(|| {
                        anyhow::anyhow!("pipeline {name} is missing from requested revision")
                    })?;
                Ok((
                    pipeline.config(),
                    &pipeline.working_directory,
                    Some(&pipeline.runner_os),
                    pipeline.sparse_view.as_deref(),
                ))
            }
            None => {
                ensure!(
                    self.pipelines.is_empty(),
                    "named pipelines require a push trigger; no pipeline was selected"
                );
                Ok((
                    PipelineConfig {
                        stages: self.stages.clone(),
                        jobs: self.jobs.clone(),
                    },
                    ".",
                    None,
                    None,
                ))
            }
        }
    }
}

fn default_timeout() -> u64 {
    3600
}

fn default_category() -> String {
    "uncategorized".into()
}

impl PipelineConfig {
    pub fn parse(source: &str) -> Result<Self> {
        ensure!(source.len() <= 256 * 1024, ".lore-ci.toml exceeds 256 KiB");
        let mut config: Self = toml::from_str(source)?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&mut self) -> Result<()> {
        let config = self;
        ensure!(
            !config.stages.is_empty() && config.stages.len() <= 32,
            "provide 1..32 stages"
        );
        let stages: HashSet<_> = config.stages.iter().collect();
        ensure!(
            stages.len() == config.stages.len(),
            "stage names must be unique"
        );
        ensure!(
            config.stages.iter().all(|s| valid_name(s)),
            "invalid stage name"
        );
        ensure!(
            !config.jobs.is_empty() && config.jobs.len() <= 128,
            "provide 1..128 jobs"
        );
        let mut names = HashSet::new();
        for job in &config.jobs {
            ensure!(
                valid_name(&job.name) && names.insert(&job.name),
                "invalid or duplicate job name"
            );
            ensure!(
                stages.contains(&job.stage),
                "unknown stage for job {}",
                job.name
            );
            ensure!(
                (1..=86400).contains(&job.timeout_seconds),
                "job timeout must be 1..86400 seconds"
            );
            ensure!(
                !job.script.is_empty()
                    && job
                        .script
                        .iter()
                        .all(|s| !s.trim().is_empty() && !s.contains('\0')),
                "job script cannot be empty or contain NUL"
            );
        }
        config
            .jobs
            .sort_by_key(|j| config.stages.iter().position(|s| s == &j.stage).unwrap());
        Ok(())
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 100
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_only_matching_paths_and_preserves_stage_order() {
        let source = include_str!("../../examples/monorepo.lore-ci.toml");
        let file = PipelineFile::parse(source).unwrap();
        let selected = |paths: &[&str]| {
            let paths = paths.iter().map(|p| p.to_string()).collect::<Vec<_>>();
            file.pipelines
                .iter()
                .filter(|p| p.matches(&paths))
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(selected(&["Client/src/main.cs"]), ["client"]);
        assert_eq!(selected(&["Server/src/main.rs"]), ["server"]);
        assert_eq!(
            selected(&["Client", "Server/src/main.rs"]),
            ["client", "server"]
        );
        assert!(selected(&["README.md", "ClientOther/file", "client/file"]).is_empty());
        assert!(file.select(None).is_err());
        let (server, directory, os, sparse_view) = file.select(Some("server")).unwrap();
        assert_eq!(directory, "Server");
        assert_eq!(os, Some("macos"));
        assert_eq!(sparse_view, None);
        assert_eq!(server.jobs[0].stage, "check");
    }

    #[test]
    fn rejects_unsafe_or_ambiguous_push_configuration() {
        let source = include_str!("../../examples/monorepo.lore-ci.toml");
        for invalid in [
            source.replace("runner_os = \"windows\"", "runner_os = \"Windows\""),
            source.replace(
                "working_directory = \"Client\"",
                "working_directory = \"../Client\"",
            ),
            source.replace("Client/**", "/Client/**"),
            source.replace("Client/**", "Client/*.cs"),
            source.replace("name = \"server\"", "name = \"client\""),
            format!("stages = ['test']\n{source}"),
        ] {
            assert!(PipelineFile::parse(&invalid).is_err());
        }
        for path in ["../x", "/x", "a//b", "a/./b", "C:/x", "a\\b"] {
            assert!(!valid_relative_path(path));
        }
        assert!(
            PipelineFile::parse(include_str!("../../examples/.lore-ci.toml"))
                .unwrap()
                .select(None)
                .is_ok()
        );
    }

    #[test]
    fn selects_and_validates_sparse_view_name() {
        let source = include_str!("../../examples/monorepo.lore-ci.toml").replacen(
            "runner_os = \"windows\"",
            "runner_os = \"windows\"\nsparse_view = \"  ServerBuildView  \"",
            1,
        );
        let file = PipelineFile::parse(&source).unwrap();
        let (_, _, _, view) = file.select(Some("client")).unwrap();
        assert_eq!(view, Some("ServerBuildView"));

        let invalid = source.replace("  ServerBuildView  ", "   ");
        assert!(PipelineFile::parse(&invalid).is_err());
    }

    #[test]
    fn submission_requires_lore_and_an_immutable_revision() {
        let mut request = SubmitPipeline {
            repository_url: "lores://localhost:41337/project".into(),
            revision: "a".repeat(64),
            branch: Some("main".into()),
            pipeline_name: None,
        };
        assert!(request.validate().is_ok());
        request.repository_url = "lore://localhost:41337/project".into();
        assert!(request.validate().is_err());
        request.repository_url = "lores://localhost:41337/project".into();
        request.revision = "main@head".into();
        assert!(request.validate().is_err());
        request.revision = "a".repeat(64);
        request.repository_url = "https://example.com/project.git".into();
        assert!(request.validate().is_err());
        request.repository_url = "lores://user:secret@localhost/project".into();
        assert!(request.validate().is_err());
        request.repository_url = "lores://localhost:41337/project".into();
        request.branch = Some("".into());
        assert!(request.validate().is_err());
        request.branch = Some("main".into());
        request.pipeline_name = Some("".into());
        assert!(request.validate().is_err());
    }

    #[test]
    fn validates_and_orders_jobs_by_stage() {
        let source = r#"
stages = ["build", "test"]
[[jobs]]
name = "test"
stage = "test"
script = ["cargo test"]
[[jobs]]
name = "build"
stage = "build"
script = ["cargo build"]
"#;
        assert_eq!(PipelineConfig::parse(source).unwrap().jobs[0].name, "build");
        assert!(
            PipelineConfig::parse(&source.replace("stage = \"test\"", "stage = \"missing\""))
                .is_err()
        );
        assert!(
            PipelineConfig::parse(&source.replace("name = \"test\"", "name = \"build\"")).is_err()
        );
        assert!(PipelineConfig::parse(&format!("{source}\ntimeout_seconds = 0")).is_err());
    }
}
