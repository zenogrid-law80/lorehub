//! Isolated UI fixture; no database, credentials, Lore calls, or persistent writes.
//! cargo run --no-default-features --example ci_visual_preview
//! Open http://127.0.0.1:4180/#ci-settings
//! Add --large to exercise long overview columns and a server pipeline with many jobs.
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use lorehub::ci::{analysis::analyze, config::PipelineFile};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

struct Versions {
    sources: Vec<String>,
    active: usize,
    lock: i64,
    layouts: std::collections::HashMap<String, serde_json::Map<String, Value>>,
}
type Fixture = Arc<Mutex<Versions>>;
fn version_id(index: usize) -> uuid::Uuid {
    uuid::Uuid::from_u128(index as u128 + 1)
}
fn config_response(versions: &Versions) -> Value {
    let content = &versions.sources[versions.active];
    json!({"branch":"main","revision":"a".repeat(64),"content":content,
        "configuration":PipelineFile::parse(content).unwrap(),"source_mode":"db",
        "config_revision_id":version_id(versions.active),"config_version":versions.active+1,"lock_version":versions.lock})
}

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
        .route("/api/v1/repositories/demo/ci-config/history", get(history))
        .route(
            "/api/v1/repositories/demo/ci-config/versions/{id}",
            get(version),
        )
        .route("/api/v1/repositories/demo/ci-config/restore", post(restore))
        .route(
            "/api/v1/repositories/demo/ci-config/layout",
            get(layout).post(save_layout),
        )
        .fallback(fixture)
        .with_state(Arc::new(Mutex::new(Versions {
            sources: vec![source.clone(), source.replace("echo test", "echo test v2")],
            active: 1,
            lock: 2,
            layouts: Default::default(),
        })));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:4180").await?;
    println!("Isolated CI fixture: http://127.0.0.1:4180/#ci-settings");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn config(State(source): State<Fixture>) -> Json<Value> {
    Json(config_response(&source.lock().unwrap()))
}

async fn layout(
    State(source): State<Fixture>,
    Query(input): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let key = format!("{}:{}", input["branch"], input["graph"]);
    Json(json!({"positions":source.lock().unwrap().layouts.get(&key).cloned().unwrap_or_default()}))
}

async fn save_layout(State(source): State<Fixture>, Json(input): Json<Value>) -> Json<Value> {
    let key = format!(
        "{}:{}",
        input["branch"].as_str().unwrap(),
        input["graph"].as_str().unwrap()
    );
    let mut state = source.lock().unwrap();
    let positions = state.layouts.entry(key).or_default();
    if input["reset"] == true {
        positions.clear();
    }
    if let Some(patch) = input["positions"].as_object() {
        for (key, value) in patch {
            if value.is_null() {
                positions.remove(key);
            } else {
                positions.insert(key.clone(), value.clone());
            }
        }
    }
    Json(json!({"positions":positions}))
}

async fn save(State(source): State<Fixture>, Json(input): Json<Value>) -> Response {
    let content = input["content"].as_str().unwrap_or("");
    if let Err(error) = PipelineFile::parse(content) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":error.to_string()})),
        )
            .into_response();
    }
    let mut state = source.lock().unwrap();
    if input["expected_lock_version"].as_i64() != Some(state.lock) {
        return (
            StatusCode::CONFLICT,
            Json(json!({"error":"CI settings changed"})),
        )
            .into_response();
    }
    state.sources.push(content.to_owned());
    state.active = state.sources.len() - 1;
    state.lock += 1;
    Json(config_response(&state)).into_response()
}

async fn history(State(source): State<Fixture>) -> Json<Value> {
    let state = source.lock().unwrap();
    Json(
        json!({"versions": (0..state.sources.len()).rev().map(|i| json!({"id":version_id(i),"version":i+1,"created_at":"2026-10-08T00:00:00Z"})).collect::<Vec<_>>(),"events":[]}),
    )
}

async fn version(State(source): State<Fixture>, Path(id): Path<uuid::Uuid>) -> Response {
    let state = source.lock().unwrap();
    let Some(index) = (0..state.sources.len()).find(|&i| version_id(i) == id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    Json(json!({"id":id,"version":index+1,"source_toml":state.sources[index],"created_at":"2026-10-08T00:00:00Z","created_by":"Preview user"})).into_response()
}

async fn restore(State(source): State<Fixture>, Json(input): Json<Value>) -> Response {
    let mut state = source.lock().unwrap();
    if input["expected_lock_version"].as_i64() != Some(state.lock) {
        return (
            StatusCode::CONFLICT,
            Json(json!({"error":"CI settings changed"})),
        )
            .into_response();
    }
    let Some(index) =
        (0..state.sources.len()).find(|&i| json!(version_id(i)) == input["revision_id"])
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    state.active = index;
    state.lock += 1;
    Json(config_response(&state)).into_response()
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
        "/assets/lore-logo.svg" => Some(("lore-logo.svg", "image/svg+xml")),
        "/assets/app.js" => Some(("app.js", "text/javascript")),
        "/assets/ci-visual.js" => Some(("ci-visual.js", "text/javascript")),
        "/assets/ci-editor.js" => Some(("ci-editor.js", "text/javascript")),
        "/assets/ci-layout.js" => Some(("ci-layout.js", "text/javascript")),
        "/assets/execution-detail.js" => Some(("execution-detail.js", "text/javascript")),
        "/assets/overview.js" => Some(("overview.js", "text/javascript")),
        "/assets/backups.js" => Some(("backups.js", "text/javascript")),
        "/assets/execution-graph.js" => Some(("execution-graph.js", "text/javascript")),
        "/assets/execution-analysis.js" => Some(("execution-analysis.js", "text/javascript")),
        "/assets/management.js" => Some(("management.js", "text/javascript")),
        "/assets/operations.js" => Some(("operations.js", "text/javascript")),
        "/assets/repository-context.js" => Some(("repository-context.js", "text/javascript")),
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
