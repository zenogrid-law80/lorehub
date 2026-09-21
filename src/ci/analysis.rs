//! Read-only analysis using the same validation and path rules as push CI.
use anyhow::{Result, ensure};
use serde::Serialize;

use super::config::{ConfigDiagnostic, PipelineFile, change_path_matches, valid_relative_path};

#[derive(Debug, Serialize)]
pub struct CiAnalysis {
    pub valid: bool,
    pub diagnostics: Vec<ConfigDiagnostic>,
    pub manual: bool,
    pub pipelines: Vec<PipelinePreview>,
}

#[derive(Debug, Serialize)]
pub struct PipelinePreview {
    pub pipeline_index: usize,
    pub name: String,
    pub matched: bool,
    pub matching_patterns: Vec<String>,
    pub matching_paths: Vec<String>,
    /// Matched pipelines referencing this prerequisite. This does not enqueue it.
    pub referenced_by: Vec<String>,
}

pub fn analyze(source: &str, paths: &[String]) -> Result<CiAnalysis> {
    ensure!(paths.len() <= 128, "provide at most 128 changed paths");
    ensure!(
        paths.iter().all(|path| path.len() <= 2048
            && valid_relative_path(path)
            && !path.chars().any(char::is_control)),
        "changed paths must be relative repository paths (at most 2048 bytes each)"
    );
    let file = match PipelineFile::parse(source) {
        Ok(file) => file,
        Err(error) => {
            return Ok(CiAnalysis {
                valid: false,
                diagnostics: vec![ConfigDiagnostic::from_error(&error, source)],
                manual: false,
                pipelines: vec![],
            });
        }
    };
    let matched: Vec<_> = file
        .pipelines
        .iter()
        .map(|pipeline| pipeline.matches(paths))
        .collect();
    let pipelines = file
        .pipelines
        .iter()
        .enumerate()
        .map(|(pipeline_index, pipeline)| PipelinePreview {
            pipeline_index,
            name: pipeline.name.clone(),
            matched: matched[pipeline_index],
            matching_patterns: pipeline
                .changes
                .iter()
                .filter(|pattern| paths.iter().any(|path| change_path_matches(pattern, path)))
                .cloned()
                .collect(),
            matching_paths: paths
                .iter()
                .filter(|path| {
                    pipeline
                        .changes
                        .iter()
                        .any(|pattern| change_path_matches(pattern, path))
                })
                .cloned()
                .collect(),
            referenced_by: file
                .pipelines
                .iter()
                .enumerate()
                .filter(|(index, candidate)| {
                    matched[*index] && candidate.needs.contains(&pipeline.name)
                })
                .map(|(_, candidate)| candidate.name.clone())
                .collect(),
        })
        .collect();
    Ok(CiAnalysis {
        valid: true,
        diagnostics: vec![],
        manual: file.pipelines.is_empty(),
        pipelines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pipeline(name: &str, changes: &str, needs: &str) -> String {
        format!(
            r#"
[[pipelines]]
name = "{name}"
runner_os = "linux"
working_directory = "."
changes = ["{changes}"]
needs = [{needs}]
stages = ["build"]
[[pipelines.jobs]]
name = "compile"
stage = "build"
script = ["echo hello"]
"#
        )
    }

    #[test]
    fn preview_matches_push_rules_without_enqueuing_dependencies() {
        let source = pipeline("tools", "Tools/**", "") + &pipeline("game", "Game/**", "\"tools\"");
        let result = analyze(
            &source,
            &[
                "Game/src/main.rs".into(),
                "Games/no.rs".into(),
                "game/no.rs".into(),
            ],
        )
        .unwrap();
        assert!(result.valid);
        assert!(!result.pipelines[0].matched);
        assert_eq!(result.pipelines[0].referenced_by, ["game"]);
        assert!(result.pipelines[1].matched);
        assert_eq!(result.pipelines[1].matching_paths, ["Game/src/main.rs"]);
        assert_eq!(result.pipelines[1].matching_patterns, ["Game/**"]);
    }

    #[test]
    fn diagnostics_locate_jobs_before_sorting_and_stages() {
        let source = pipeline("game", "Game/**", "");
        for (bad, field, job, stage) in [
            (
                source.replace("script = [\"echo hello\"]", "script = []"),
                "script",
                Some(0),
                None,
            ),
            (
                source.clone() + "timeout_seconds = 0\n",
                "timeout_seconds",
                Some(0),
                None,
            ),
            (
                source.replace("stages = [\"build\"]", "stages = [\"build\", \"build\"]"),
                "name",
                None,
                Some(1),
            ),
            (source.replace("Game/**", "**"), "changes", None, None),
        ] {
            let result = analyze(&bad, &[]).unwrap();
            assert!(!result.valid);
            let issue = &result.diagnostics[0];
            assert_eq!(issue.location.pipeline_index, Some(0));
            assert_eq!(issue.location.job_index, job);
            assert_eq!(issue.location.stage_index, stage);
            assert_eq!(issue.location.field, field);
        }
    }

    #[test]
    fn dependencies_report_their_owner_and_cycles_are_rejected() {
        let unknown = analyze(&pipeline("game", "Game/**", "\"missing\""), &[]).unwrap();
        assert_eq!(unknown.diagnostics[0].location.pipeline_index, Some(0));
        assert_eq!(unknown.diagnostics[0].location.field, "needs");
        let cycle = pipeline("a", "a/**", "\"b\"") + &pipeline("b", "b/**", "\"a\"");
        assert!(!analyze(&cycle, &[]).unwrap().valid);
        let downstream_first = pipeline("dependent", "dependent/**", "\"a\"") + &cycle;
        let cycle_issue = &analyze(&downstream_first, &[]).unwrap().diagnostics[0];
        assert!(matches!(cycle_issue.location.pipeline_index, Some(1 | 2)));
        let bad_job = pipeline("game", "Game/**", "") + "needs = [\"missing\"]\n";
        let issue = &analyze(&bad_job, &[]).unwrap().diagnostics[0];
        assert_eq!(issue.location.job_index, Some(0));
        assert_eq!(issue.location.field, "needs");
    }

    #[test]
    fn syntax_and_manual_and_limits() {
        let result = analyze("# 한글\nstages = [", &[]).unwrap();
        assert!(!result.valid);
        assert_eq!(result.diagnostics[0].line, Some(2));
        let manual = "stages = [\"build\"]\n[[jobs]]\nname = \"job\"\nstage = \"build\"\nscript = [\"echo ok\"]";
        assert!(analyze(manual, &["Game/file".into()]).unwrap().manual);
        assert!(analyze(manual, &["../outside".into()]).is_err());
        assert!(analyze(manual, &vec!["a".into(); 129]).is_err());
    }
}
