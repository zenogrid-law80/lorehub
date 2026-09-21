use std::collections::{HashMap, HashSet};

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Indices refer to the submitted document, before jobs are topologically sorted.
#[derive(Debug, Default, Clone, Serialize)]
pub struct ConfigLocation {
    pub pipeline_index: Option<usize>,
    pub job_index: Option<usize>,
    pub stage_index: Option<usize>,
    pub field: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigDiagnostic {
    pub message: String,
    #[serde(flatten)]
    pub location: ConfigLocation,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl std::fmt::Display for ConfigDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ConfigDiagnostic {}

impl ConfigDiagnostic {
    fn at(error: anyhow::Error, location: ConfigLocation) -> anyhow::Error {
        let mut diagnostic = error.downcast_ref::<Self>().cloned().unwrap_or(Self {
            message: error.to_string(),
            location: location.clone(),
            line: None,
            column: None,
        });
        if location.pipeline_index.is_some() {
            diagnostic.location.pipeline_index = location.pipeline_index;
        }
        diagnostic.into()
    }

    pub fn from_error(error: &anyhow::Error, source: &str) -> Self {
        if let Some(diagnostic) = error.downcast_ref::<Self>() {
            return diagnostic.clone();
        }
        let mut diagnostic = Self {
            message: error.to_string(),
            location: ConfigLocation::default(),
            line: None,
            column: None,
        };
        if let Some(error) = error.downcast_ref::<toml::de::Error>()
            && let Some(span) = error.span()
        {
            let mut offset = span.start.min(source.len());
            while !source.is_char_boundary(offset) {
                offset -= 1;
            }
            let prefix = &source[..offset];
            diagnostic.line = Some(prefix.bytes().filter(|b| *b == b'\n').count() + 1);
            diagnostic.column = Some(prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1);
        }
        diagnostic
    }
}

macro_rules! ensure_config {
    ($condition:expr, $location:expr, $($message:tt)*) => {
        if !$condition {
            return Err(ConfigDiagnostic::at(anyhow::anyhow!($($message)*), $location));
        }
    };
}

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
    #[serde(default)]
    pub needs: Vec<String>,
    pub script: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

/// A repository can keep the original single pipeline, or opt into push CI.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct PipelineFile {
    #[serde(default)]
    stages: Vec<String>,
    #[serde(default)]
    jobs: Vec<JobConfig>,
    #[serde(default)]
    pub pipelines: Vec<NamedPipeline>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct NamedPipeline {
    pub name: String,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub needs: Vec<String>,
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
        self.changes
            .iter()
            .any(|pattern| paths.iter().any(|path| change_path_matches(pattern, path)))
    }
}

pub fn change_path_matches(pattern: &str, path: &str) -> bool {
    if let Some(directory) = pattern.strip_suffix("/**") {
        path == directory
            || path
                .strip_prefix(directory)
                .is_some_and(|tail| tail.starts_with('/'))
    } else {
        path == pattern
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
            for (index, pipeline) in file.pipelines.iter_mut().enumerate() {
                let location = |field| ConfigLocation {
                    pipeline_index: Some(index),
                    field,
                    ..Default::default()
                };
                ensure_config!(
                    valid_name(&pipeline.name) && names.insert(pipeline.name.clone()),
                    location("name"),
                    "invalid or duplicate pipeline name"
                );
                ensure_config!(
                    valid_name(&pipeline.category),
                    location("category"),
                    "invalid pipeline category"
                );
                ensure_config!(
                    matches!(pipeline.runner_os.as_str(), "windows" | "macos" | "linux"),
                    location("runner_os"),
                    "runner_os must be windows, macos or linux"
                );
                if let Some(view) = pipeline.sparse_view.as_mut() {
                    *view = view.trim().to_owned();
                    ensure_config!(
                        !view.is_empty()
                            && view.len() <= 100
                            && !view.chars().any(char::is_control),
                        location("sparse_view"),
                        "sparse_view must be 1 to 100 characters without control characters"
                    );
                }
                ensure_config!(
                    pipeline.working_directory == "."
                        || valid_relative_path(&pipeline.working_directory),
                    location("working_directory"),
                    "working_directory must stay inside the checkout"
                );
                ensure_config!(
                    !pipeline.changes.is_empty() && pipeline.changes.len() <= 128,
                    location("changes"),
                    "provide 1..128 change paths"
                );
                for pattern in &pipeline.changes {
                    let path = pattern.strip_suffix("/**").unwrap_or(pattern);
                    ensure_config!(
                        valid_relative_path(path) && !path.contains(['*', '?', '[', ']']),
                        location("changes"),
                        "changes must contain relative exact paths or directory/** patterns"
                    );
                }
                let mut config = pipeline.config();
                config
                    .validate()
                    .map_err(|error| ConfigDiagnostic::at(error, location("")))?;
                pipeline.jobs = config.jobs;
            }
            let pipeline_indices = file
                .pipelines
                .iter()
                .enumerate()
                .map(|(index, pipeline)| (pipeline.name.as_str(), index))
                .collect::<HashMap<_, _>>();
            let mut dependents = vec![Vec::new(); file.pipelines.len()];
            let mut prerequisites = vec![Vec::new(); file.pipelines.len()];
            let mut indegrees = vec![0usize; file.pipelines.len()];
            for (index, pipeline) in file.pipelines.iter().enumerate() {
                let location = ConfigLocation {
                    pipeline_index: Some(index),
                    field: "needs",
                    ..Default::default()
                };
                ensure_config!(
                    pipeline.needs.len() <= 32,
                    location.clone(),
                    "provide at most 32 pipeline dependencies"
                );
                let mut dependencies = HashSet::new();
                for dependency in &pipeline.needs {
                    ensure_config!(
                        dependency != &pipeline.name && dependencies.insert(dependency),
                        location.clone(),
                        "pipeline {} has a duplicate or self dependency",
                        pipeline.name
                    );
                    let dependency_index =
                        *pipeline_indices.get(dependency.as_str()).ok_or_else(|| {
                            ConfigDiagnostic::at(
                                anyhow::anyhow!(
                                    "pipeline {} depends on unknown pipeline {}",
                                    pipeline.name,
                                    dependency
                                ),
                                location.clone(),
                            )
                        })?;
                    dependents[dependency_index].push(index);
                    prerequisites[index].push(dependency_index);
                    indegrees[index] += 1;
                }
            }
            let mut emitted = vec![false; file.pipelines.len()];
            for _ in 0..file.pipelines.len() {
                let Some(next) = (0..file.pipelines.len())
                    .find(|&index| !emitted[index] && indegrees[index] == 0)
                else {
                    return Err(ConfigDiagnostic::at(
                        anyhow::anyhow!("pipeline dependencies must not contain a cycle"),
                        ConfigLocation {
                            pipeline_index: cycle_member(&prerequisites, &emitted),
                            field: "needs",
                            ..Default::default()
                        },
                    ));
                };
                emitted[next] = true;
                for &dependent in &dependents[next] {
                    indegrees[dependent] -= 1;
                }
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

    pub(crate) fn validate(&mut self) -> Result<()> {
        let config = self;
        ensure_config!(
            !config.stages.is_empty() && config.stages.len() <= 32,
            ConfigLocation {
                field: "stages",
                ..Default::default()
            },
            "provide 1..32 stages"
        );
        let mut stages = HashSet::new();
        for (index, stage) in config.stages.iter().enumerate() {
            let location = ConfigLocation {
                stage_index: Some(index),
                field: "name",
                ..Default::default()
            };
            ensure_config!(
                stages.insert(stage),
                location.clone(),
                "stage names must be unique"
            );
            ensure_config!(valid_name(stage), location, "invalid stage name");
        }
        ensure_config!(
            !config.jobs.is_empty() && config.jobs.len() <= 128,
            ConfigLocation {
                field: "jobs",
                ..Default::default()
            },
            "provide 1..128 jobs"
        );
        let mut names = HashSet::new();
        for (index, job) in config.jobs.iter().enumerate() {
            let location = |field| ConfigLocation {
                job_index: Some(index),
                field,
                ..Default::default()
            };
            ensure_config!(
                valid_name(&job.name) && names.insert(&job.name),
                location("name"),
                "invalid or duplicate job name"
            );
            ensure_config!(
                stages.contains(&job.stage),
                location("stage"),
                "unknown stage for job {}",
                job.name
            );
            ensure_config!(
                (1..=86400).contains(&job.timeout_seconds),
                location("timeout_seconds"),
                "job timeout must be 1..86400 seconds"
            );
            ensure_config!(
                !job.script.is_empty()
                    && job
                        .script
                        .iter()
                        .all(|s| !s.trim().is_empty() && !s.contains('\0')),
                location("script"),
                "job script cannot be empty or contain NUL"
            );
        }
        let job_indices = config
            .jobs
            .iter()
            .enumerate()
            .map(|(index, job)| (job.name.clone(), index))
            .collect::<HashMap<_, _>>();
        let stage_indices = config
            .stages
            .iter()
            .enumerate()
            .map(|(index, stage)| (stage.as_str(), index))
            .collect::<HashMap<_, _>>();
        let mut dependents = vec![Vec::new(); config.jobs.len()];
        let mut prerequisites = vec![Vec::new(); config.jobs.len()];
        let mut indegrees = vec![0usize; config.jobs.len()];
        for (index, job) in config.jobs.iter().enumerate() {
            let location = ConfigLocation {
                job_index: Some(index),
                field: "needs",
                ..Default::default()
            };
            ensure_config!(
                job.needs.len() <= 128,
                location.clone(),
                "provide at most 128 job dependencies"
            );
            let mut needs = HashSet::new();
            for dependency in &job.needs {
                ensure_config!(
                    dependency != &job.name && needs.insert(dependency),
                    location.clone(),
                    "job {} has a duplicate or self dependency",
                    job.name
                );
                let dependency_index = *job_indices.get(dependency).ok_or_else(|| {
                    ConfigDiagnostic::at(
                        anyhow::anyhow!("job {} depends on unknown job {}", job.name, dependency),
                        location.clone(),
                    )
                })?;
                ensure_config!(
                    stage_indices[config.jobs[dependency_index].stage.as_str()]
                        <= stage_indices[job.stage.as_str()],
                    location.clone(),
                    "job {} cannot depend on later-stage job {}",
                    job.name,
                    dependency
                );
                dependents[dependency_index].push(index);
                prerequisites[index].push(dependency_index);
                indegrees[index] += 1;
            }
        }
        let mut order = Vec::with_capacity(config.jobs.len());
        let mut emitted = vec![false; config.jobs.len()];
        while order.len() < config.jobs.len() {
            let next = (0..config.jobs.len())
                .filter(|&index| !emitted[index] && indegrees[index] == 0)
                .min_by_key(|&index| (stage_indices[config.jobs[index].stage.as_str()], index));
            let Some(next) = next else {
                return Err(ConfigDiagnostic::at(
                    anyhow::anyhow!("job dependencies must not contain a cycle"),
                    ConfigLocation {
                        job_index: cycle_member(&prerequisites, &emitted),
                        field: "needs",
                        ..Default::default()
                    },
                ));
            };
            emitted[next] = true;
            order.push(next);
            for &dependent in &dependents[next] {
                indegrees[dependent] -= 1;
            }
        }
        let jobs = config.jobs.clone();
        config.jobs = order.into_iter().map(|index| jobs[index].clone()).collect();
        Ok(())
    }
}

// Walk upstream from a blocked node; the repeated node is in the cycle, not just downstream of it.
fn cycle_member(prerequisites: &[Vec<usize>], emitted: &[bool]) -> Option<usize> {
    let mut node = emitted.iter().position(|done| !done)?;
    let mut visited = HashSet::new();
    while visited.insert(node) {
        node = *prerequisites[node].iter().find(|&&index| !emitted[index])?;
    }
    Some(node)
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
needs = ["build"]
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

    #[test]
    fn validates_and_orders_job_dependencies() {
        let source = r#"
stages = ["build", "test"]
[[jobs]]
name = "package"
stage = "build"
needs = ["compile"]
script = ["cargo build"]
[[jobs]]
name = "compile"
stage = "build"
script = ["cargo check"]
[[jobs]]
name = "test"
stage = "test"
needs = ["package"]
script = ["cargo test"]
"#;
        let config = PipelineConfig::parse(source).unwrap();
        assert_eq!(
            config
                .jobs
                .iter()
                .map(|job| job.name.as_str())
                .collect::<Vec<_>>(),
            ["compile", "package", "test"]
        );
        assert!(
            PipelineConfig::parse(
                &source.replace("needs = [\"compile\"]", "needs = [\"missing\"]")
            )
            .is_err()
        );
        assert!(
            PipelineConfig::parse(&source.replace(
                "name = \"compile\"\nstage = \"build\"",
                "name = \"compile\"\nstage = \"build\"\nneeds = [\"package\"]"
            ))
            .is_err()
        );
        assert!(
            PipelineConfig::parse(&source.replace("needs = [\"compile\"]", "needs = [\"test\"]"))
                .is_err()
        );
    }

    #[test]
    fn validates_pipeline_dependencies() {
        let source = r#"
[[pipelines]]
name = "data-table-generate"
runner_os = "linux"
changes = ["data/**"]
working_directory = "."
stages = ["generate"]
[[pipelines.jobs]]
name = "generate"
stage = "generate"
script = ["./generate.sh"]

[[pipelines]]
name = "server-windows-build"
needs = ["data-table-generate"]
runner_os = "windows"
changes = ["Server/**"]
working_directory = "Server"
stages = ["build"]
[[pipelines.jobs]]
name = "build"
stage = "build"
script = ["./build.ps1"]

[[pipelines]]
name = "server"
needs = ["data-table-generate"]
runner_os = "linux"
changes = ["Server/**"]
working_directory = "Server"
stages = ["build"]
[[pipelines.jobs]]
name = "build"
stage = "build"
script = ["cargo build"]
"#;
        let file = PipelineFile::parse(source).unwrap();
        assert_eq!(file.pipelines[1].needs, ["data-table-generate"]);
        assert_eq!(file.pipelines[2].needs, ["data-table-generate"]);
        assert!(
            PipelineFile::parse(
                &source.replace("needs = [\"data-table-generate\"]", "needs = [\"missing\"]")
            )
            .is_err()
        );
        let cycle = source.replace(
            "name = \"data-table-generate\"\nrunner_os",
            "name = \"data-table-generate\"\nneeds = [\"server\"]\nrunner_os",
        );
        assert!(PipelineFile::parse(&cycle).is_err());
    }

    #[test]
    fn serializes_the_validated_model_for_visual_editing() {
        let manual = PipelineFile::parse(include_str!("../../examples/.lore-ci.toml")).unwrap();
        let manual_json = serde_json::to_value(manual).unwrap();
        assert!(
            manual_json["stages"]
                .as_array()
                .is_some_and(|stages| !stages.is_empty())
        );
        assert!(
            manual_json["jobs"]
                .as_array()
                .is_some_and(|jobs| !jobs.is_empty())
        );

        let named =
            PipelineFile::parse(include_str!("../../examples/monorepo.lore-ci.toml")).unwrap();
        let named_json = serde_json::to_value(named).unwrap();
        assert_eq!(named_json["stages"], serde_json::json!([]));
        assert_eq!(named_json["jobs"], serde_json::json!([]));
        assert_eq!(named_json["pipelines"][0]["name"], "client");
        assert!(named_json["pipelines"][0]["jobs"].is_array());
    }
}
