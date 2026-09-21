//! Isolated UI fixture; no database, credentials, Lore calls, or persistent writes.
//! cargo run --no-default-features --example ci_visual_preview
//! Open http://127.0.0.1:4180/#ci-settings
//! Add --large to exercise long overview columns and a server pipeline with many jobs.
use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use lorehub::ci::{analysis::analyze, config::PipelineFile};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

type Fixture = Arc<Mutex<String>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut source = String::new();
    for (name, os, path, needs) in [
        ("tools-build", "linux", "Tools/**", ""),
        ("game-build", "windows", "Game/**", "\"tools-build\""),
        ("release", "macos", "Release/**", "\"game-build\""),
    ] {
        source += &format!(
            r#"
[[pipelines]]
name = "{name}"
category = "build"
needs = [{needs}]
runner_os = "{os}"
working_directory = "."
changes = ["{path}"]
stages = ["build", "test"]
[[pipelines.jobs]]
name = "compile"
stage = "build"
script = ["echo compile"]
[[pipelines.jobs]]
name = "test"
stage = "test"
needs = ["compile"]
script = ["echo test"]
"#
        );
    }
    if std::env::args().any(|arg| arg == "--large") {
        for index in 1..=8 {
            source += &format!(
                r#"
[[pipelines]]
name = "client-package-{index}"
runner_os = "windows"
working_directory = "."
changes = ["Client/**"]
stages = ["package"]
[[pipelines.jobs]]
name = "package"
stage = "package"
script = ["echo package"]
"#
            );
        }
        source += r#"
[[pipelines]]
name = "server"
needs = ["tools-build"]
runner_os = "macos"
working_directory = "Server"
changes = ["Server/**"]
stages = ["build", "test", "package", "publish"]
"#;
        for index in 1..=12 {
            source += &format!(
                r#"
[[pipelines.jobs]]
name = "build-{index}"
stage = "build"
script = ["echo build {index}"]
"#
            );
        }
        for (stage, needs) in [
            ("test", "build-12"),
            ("package", "test"),
            ("publish", "package"),
        ] {
            source += &format!(
                r#"
[[pipelines.jobs]]
name = "{stage}"
stage = "{stage}"
needs = ["{needs}"]
script = ["echo {stage}"]
"#
            );
        }
    }
    PipelineFile::parse(&source)?;
    let app = Router::new()
        .route(
            "/api/v1/repositories/demo/ci-config",
            get(config).post(save),
        )
        .route(
            "/api/v1/repositories/demo/ci-config/analyze",
            post(analysis),
        )
        .route("/api/v1/repositories/demo/ci-config/parse", post(parse))
        .fallback(fixture)
        .with_state(Arc::new(Mutex::new(source)));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:4180").await?;
    println!("Isolated CI fixture: http://127.0.0.1:4180/#ci-settings");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn config(State(source): State<Fixture>) -> Json<Value> {
    let content = source.lock().unwrap().clone();
    Json(
        json!({"branch": "main", "revision": "a".repeat(64), "configuration": PipelineFile::parse(&content).unwrap(), "content": content}),
    )
}

async fn save(State(source): State<Fixture>, Json(input): Json<Value>) -> Response {
    let content = input["content"].as_str().unwrap_or("");
    match PipelineFile::parse(content) {
        Ok(configuration) => {
            *source.lock().unwrap() = content.to_string();
            Json(json!({"branch": "main", "revision": "a".repeat(64), "configuration": configuration, "content": content})).into_response()
        }
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

async fn analysis(Json(input): Json<Value>) -> Response {
    let paths: Vec<String> =
        serde_json::from_value(input.get("changed_paths").cloned().unwrap_or(json!([]))).unwrap();
    match analyze(input["content"].as_str().unwrap_or(""), &paths) {
        Ok(result) => Json(result).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

async fn parse(Json(input): Json<Value>) -> Response {
    match PipelineFile::parse(input["content"].as_str().unwrap_or("")) {
        Ok(result) => Json(result).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

async fn fixture(uri: Uri) -> Response {
    let path = uri.path();
    let asset = match path {
        "/" => Some(("index.html", "text/html; charset=utf-8")),
        "/assets/app.js" => Some(("app.js", "text/javascript")),
        "/assets/ci-visual.js" => Some(("ci-visual.js", "text/javascript")),
        "/assets/ci-editor.js" => Some(("ci-editor.js", "text/javascript")),
        "/assets/execution-graph.js" => Some(("execution-graph.js", "text/javascript")),
        "/assets/management.js" => Some(("management.js", "text/javascript")),
        "/assets/theme.js" => Some(("theme.js", "text/javascript")),
        "/assets/app.css" => Some(("app.css", "text/css")),
        _ => None,
    };
    if let Some((file, content_type)) = asset {
        let content =
            tokio::fs::read_to_string(format!("{}/web/{file}", env!("CARGO_MANIFEST_DIR")))
                .await
                .unwrap();
        return (
            [
                ("content-type", content_type),
                ("cache-control", "no-store"),
                (
                    "content-security-policy",
                    "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:",
                ),
            ],
            content,
        )
            .into_response();
    }
    Json(match path {
        "/api/v1/me" => json!({"id":"fixture", "name":"CI Preview", "email":"fixture@example.test", "role":"user"}),
        "/api/v1/repositories" => json!({"repositories":[{"id":"demo", "name":"demo", "url":"lores://fixture/demo", "storage_backend":"dynamodb_s3"}], "server_url":"lores://fixture", "storage_backends":["dynamodb_s3"]}),
        "/api/v1/pipeline-history" => json!({"pipelines":[], "next_before":null}),
        "/api/v1/repositories/demo/branches" => json!([{"name":"main", "revision":"a".repeat(64)}]),
        _ => json!([]),
    }).into_response()
}
