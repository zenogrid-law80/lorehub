//! Embedded LoreHub web application assets.

use axum::{body::Body, http::header, response::Response};

const INDEX: &str = include_str!("../../web/index.html");
const STYLES: &str = include_str!("../../web/app.css");
const THEME_SCRIPT: &str = include_str!("../../web/theme.js");
const SCRIPT: &str = include_str!("../../web/app.js");
const CI_SCRIPT: &str = include_str!("../../web/ci-visual.js");
const CI_EDITOR_SCRIPT: &str = include_str!("../../web/ci-editor.js");
const EXECUTION_SCRIPT: &str = include_str!("../../web/execution-graph.js");
const EXECUTION_ANALYSIS_SCRIPT: &str = include_str!("../../web/execution-analysis.js");
const REPOSITORY_CONTEXT_SCRIPT: &str = include_str!("../../web/repository-context.js");
const OPERATIONS_SCRIPT: &str = include_str!("../../web/operations.js");
const MANAGEMENT_SCRIPT: &str = include_str!("../../web/management.js");
#[cfg(feature = "embedded-runner-installers")]
const LINUX_RUNNER: &[u8] =
    include_bytes!("../../deploy/downloads/lorehub-runner-linux-x86_64-v0.1.0.tar.gz");
#[cfg(not(feature = "embedded-runner-installers"))]
const LINUX_RUNNER: &[u8] = &[];
#[cfg(feature = "embedded-runner-installers")]
const WINDOWS_RUNNER: &[u8] =
    include_bytes!("../../deploy/downloads/lorehub-runner-windows-x86_64-v0.2.21.msi");
#[cfg(not(feature = "embedded-runner-installers"))]
const WINDOWS_RUNNER: &[u8] = &[];
#[cfg(feature = "embedded-runner-installers")]
const MACOS_RUNNER: &[u8] =
    include_bytes!("../../deploy/downloads/lorehub-runner-macos-aarch64-v0.2.21.tar.gz");
#[cfg(not(feature = "embedded-runner-installers"))]
const MACOS_RUNNER: &[u8] = &[];

const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; connect-src 'self'; img-src 'self' data: https://*.googleusercontent.com; style-src 'self'; script-src 'self'; font-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self' https://accounts.google.com";

pub async fn index() -> Response {
    asset(INDEX, "text/html; charset=utf-8", true)
}

pub async fn logo() -> Response {
    asset(
        include_str!("../../web/lore-logo.svg"),
        "image/svg+xml",
        false,
    )
}

pub async fn styles() -> Response {
    asset(STYLES, "text/css; charset=utf-8", false)
}

pub async fn script() -> Response {
    asset(SCRIPT, "text/javascript; charset=utf-8", false)
}

pub async fn ci_script() -> Response {
    asset(CI_SCRIPT, "text/javascript; charset=utf-8", false)
}

pub async fn ci_editor_script() -> Response {
    asset(CI_EDITOR_SCRIPT, "text/javascript; charset=utf-8", false)
}

pub async fn execution_script() -> Response {
    asset(EXECUTION_SCRIPT, "text/javascript; charset=utf-8", false)
}

pub async fn execution_analysis_script() -> Response {
    asset(
        EXECUTION_ANALYSIS_SCRIPT,
        "text/javascript; charset=utf-8",
        false,
    )
}

pub async fn theme_script() -> Response {
    asset(THEME_SCRIPT, "text/javascript; charset=utf-8", false)
}

pub async fn repository_context_script() -> Response {
    asset(
        REPOSITORY_CONTEXT_SCRIPT,
        "text/javascript; charset=utf-8",
        false,
    )
}

pub async fn operations_script() -> Response {
    asset(OPERATIONS_SCRIPT, "text/javascript; charset=utf-8", false)
}

pub async fn overview_script() -> Response {
    asset(
        include_str!("../../web/overview.js"),
        "text/javascript; charset=utf-8",
        false,
    )
}

pub async fn management_script() -> Response {
    asset(MANAGEMENT_SCRIPT, "text/javascript; charset=utf-8", false)
}

pub async fn linux_runner() -> Response {
    download(
        LINUX_RUNNER,
        "application/gzip",
        "lorehub-runner-linux-x86_64-v0.1.0.tar.gz",
    )
}

pub async fn windows_runner() -> Response {
    download(
        WINDOWS_RUNNER,
        "application/x-msi",
        "lorehub-runner-windows-x86_64-v0.2.21.msi",
    )
}

pub async fn macos_runner() -> Response {
    download(
        MACOS_RUNNER,
        "application/gzip",
        "lorehub-runner-macos-aarch64-v0.2.21.tar.gz",
    )
}

fn download(
    content: &'static [u8],
    content_type: &'static str,
    filename: &'static str,
) -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .header(header::CONTENT_LENGTH, content.len())
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(Body::from(content))
        .expect("download response headers are valid")
}

fn asset(content: &'static str, content_type: &'static str, html: bool) -> Response {
    let mut builder = Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::REFERRER_POLICY, "no-referrer")
        .header(
            "permissions-policy",
            "camera=(), microphone=(), geolocation=()",
        )
        .header("cross-origin-opener-policy", "same-origin");
    if html {
        builder = builder
            .header("content-security-policy", CONTENT_SECURITY_POLICY)
            .header("x-frame-options", "DENY");
    }
    builder
        .body(Body::from(content))
        .expect("static response headers are valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn page_has_strict_browser_headers() {
        let response = index().await;
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
        assert!(
            response.headers()["content-security-policy"]
                .to_str()
                .unwrap()
                .contains("script-src 'self'")
        );
        assert_eq!(response.headers()["x-frame-options"], "DENY");
        assert!(INDEX.contains("id=\"runners-page\""));
        assert!(INDEX.contains("id=\"graphs-page\""));
        assert!(INDEX.contains("href=\"#graphs\""));
        assert!(INDEX.contains("id=\"ci-settings-page\""));
        assert!(INDEX.contains("href=\"#ci-settings\""));
        assert!(INDEX.contains("id=\"repository-config-repository\""));
        assert!(!INDEX.contains("id=\"repository-config-dialog\""));
        assert!(SCRIPT.contains("loadCiSettings"));
        assert!(INDEX.contains("id=\"pipelines-heading\""));
        assert!(INDEX.contains("id=\"pipeline-repository-filter\""));
        assert!(INDEX.contains("id=\"pipeline-branch-filter\""));
        assert!(INDEX.contains("id=\"pipeline-name-filter\""));
        assert!(INDEX.contains("id=\"pipeline-name\""));
        assert!(INDEX.contains("Storage Backend"));
        assert!(INDEX.contains("value=\"dynamodb_s3\""));
        assert!(INDEX.contains("value=\"local_file\""));
        assert!(SCRIPT.contains("updatePipelineFilterOptions"));
        assert!(INDEX.contains("Execution graphs"));
        let workspace_group = INDEX.find("data-nav-group=\"workspace\"").unwrap();
        let repository_group = INDEX.find("data-nav-group=\"repository\"").unwrap();
        let management_group = INDEX.find("data-nav-group=\"management\"").unwrap();
        assert!(workspace_group < repository_group && repository_group < management_group);
        assert!(MANAGEMENT_SCRIPT.contains("viewMenu"));
        assert!(MANAGEMENT_SCRIPT.contains("views: [\"Sparse View\", \"Sparse View\""));
        assert!(MANAGEMENT_SCRIPT.contains("/api/v1/sparse-views"));
        assert!(SCRIPT.contains("/api/v1/runners"));
        assert!(SCRIPT.contains("runner.docker_available"));
        assert!(INDEX.contains("<th>Docker</th>"));
        assert!(SCRIPT.contains("method: \"DELETE\""));
        assert!(INDEX.contains("id=\"remove-runner-dialog\""));
        assert!(INDEX.contains("id=\"theme-select\""));
        assert!(INDEX.contains("value=\"system\""));
        assert!(INDEX.contains("value=\"light\""));
        assert!(INDEX.contains("value=\"dark\""));
        assert!(THEME_SCRIPT.contains("prefers-color-scheme: dark"));
        assert!(THEME_SCRIPT.contains("lorehub_theme"));
        assert!(SCRIPT.contains("/api/v1/pipeline-graphs"));
        assert!(SCRIPT.contains("loadPipelineGraphs"));
        assert!(INDEX.contains("/assets/execution-graph.js"));
        assert!(INDEX.contains("/assets/execution-analysis.js"));
        assert!(EXECUTION_ANALYSIS_SCRIPT.contains("renderExecutionAnalysis"));
        assert!(EXECUTION_SCRIPT.contains("renderExecutionWorkspace"));
        assert_eq!(
            execution_script().await.headers()[header::CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
        assert!(SCRIPT.contains("graph-repository-tree"));
        assert!(SCRIPT.contains("graph-branch-tree"));
        assert!(SCRIPT.contains("graphExpandedRepositories: new Set()"));
        assert!(SCRIPT.contains("state.graphExpandedCategories.has(categoryKey)"));
        assert!(INDEX.contains("/downloads/runners/linux-x86_64"));
        assert!(INDEX.contains("/downloads/runners/windows-x86_64"));
        assert!(INDEX.contains("/downloads/runners/macos-aarch64"));
        #[cfg(feature = "embedded-runner-installers")]
        {
            assert!(!WINDOWS_RUNNER.is_empty());
            assert!(!MACOS_RUNNER.is_empty());
        }

        let response = linux_runner().await;
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/gzip");
        assert_eq!(
            response.headers()[header::CONTENT_DISPOSITION],
            "attachment; filename=\"lorehub-runner-linux-x86_64-v0.1.0.tar.gz\""
        );
        assert_eq!(
            response.headers()[header::CONTENT_LENGTH],
            LINUX_RUNNER.len().to_string()
        );

        let response = windows_runner().await;
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "application/x-msi"
        );
        assert_eq!(
            response.headers()[header::CONTENT_DISPOSITION],
            "attachment; filename=\"lorehub-runner-windows-x86_64-v0.2.21.msi\""
        );

        let response = macos_runner().await;
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/gzip");
        assert_eq!(
            response.headers()[header::CONTENT_DISPOSITION],
            "attachment; filename=\"lorehub-runner-macos-aarch64-v0.2.21.tar.gz\""
        );
    }
}

pub async fn repository_tree_script() -> Response {
    asset(
        include_str!("../../web/repository-tree.js"),
        "text/javascript; charset=utf-8",
        false,
    )
}
