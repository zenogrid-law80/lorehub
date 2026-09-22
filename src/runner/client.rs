use std::{fmt, time::Duration};

use anyhow::{Context, Result};
use reqwest::{Client, Method, Response};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

#[derive(Debug)]
struct CoordinatorError {
    status: reqwest::StatusCode,
    message: String,
}

impl fmt::Display for CoordinatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "coordinator request failed ({}): {}",
            self.status, self.message
        )
    }
}

impl std::error::Error for CoordinatorError {}

pub(crate) fn retryable(error: &anyhow::Error) -> bool {
    if let Some(error) = error.downcast_ref::<CoordinatorError>() {
        return matches!(error.status.as_u16(), 408 | 429 | 500 | 502 | 503 | 504);
    }
    error.downcast_ref::<reqwest::Error>().is_some_and(|error| {
        error.is_connect() || error.is_timeout() || error.is_body() || error.is_request()
    })
}

use crate::{
    ci::{
        config::PipelineConfig,
        db::{Job, Pipeline},
    },
    server::tokens::TokenIssuer,
};

#[derive(Clone)]
pub struct CoordinatorClient {
    client: Client,
    base_url: url::Url,
    worker: Uuid,
    issuer: TokenIssuer,
}

#[derive(Serialize)]
struct RegisterRequest<'a> {
    name: &'a str,
    os: &'a str,
    arch: &'a str,
    version: &'a str,
    docker_available: bool,
}

#[derive(Serialize)]
struct FinishRequest<'a> {
    status: &'a str,
    error: Option<&'a str>,
}

#[derive(Serialize)]
struct JobStatusRequest<'a> {
    status: &'a str,
    code: Option<i32>,
}

#[derive(Serialize)]
struct LogRequest<'a> {
    pipeline: Uuid,
    job: Option<Uuid>,
    stream: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct BoolResponse {
    active: bool,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

impl CoordinatorClient {
    pub fn new(base_url: &str, worker: Uuid, issuer: TokenIssuer) -> Result<Self> {
        let mut base_url = url::Url::parse(base_url).context("parse coordinator URL")?;
        anyhow::ensure!(
            matches!(base_url.scheme(), "https" | "http"),
            "coordinator URL must use HTTP or HTTPS"
        );
        anyhow::ensure!(
            base_url.host_str().is_some(),
            "coordinator URL requires a host"
        );
        anyhow::ensure!(
            base_url.scheme() == "https"
                || matches!(base_url.host_str(), Some("127.0.0.1" | "::1" | "localhost")),
            "coordinator URL must use HTTPS except on loopback"
        );
        base_url.set_path("/");
        base_url.set_query(None);
        base_url.set_fragment(None);
        Ok(Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()?,
            base_url,
            worker,
            issuer,
        })
    }

    async fn request(&self, method: Method, path: &str) -> Result<reqwest::RequestBuilder> {
        let token = self
            .issuer
            .issue_runner_update(&self.worker.to_string())?
            .access_token;
        Ok(self
            .client
            .request(method, self.base_url.join(path)?)
            .bearer_auth(token))
    }

    async fn checked(response: Response) -> Result<Response> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let message = response.text().await.unwrap_or_default();
        Err(CoordinatorError { status, message }.into())
    }

    async fn post_empty(&self, path: &str) -> Result<()> {
        Self::checked(self.request(Method::POST, path).await?.send().await?).await?;
        Ok(())
    }

    async fn post_json<B: Serialize + ?Sized, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<R> {
        self.post_json_once(path, body, Duration::from_secs(30))
            .await
    }

    async fn post_json_once<B: Serialize + ?Sized, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        timeout: Duration,
    ) -> Result<R> {
        Self::checked(
            self.request(Method::POST, path)
                .await?
                .timeout(timeout)
                .json(body)
                .send()
                .await?,
        )
        .await?
        .json()
        .await
        .context("decode coordinator response")
    }

    // Opt-in only: append-only logs and job creation must never be replayed.
    async fn post_retryable<B: Serialize + ?Sized, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<R> {
        for attempt in 0..4 {
            match self
                .post_json_once(path, body, Duration::from_secs(5))
                .await
            {
                Ok(response) => return Ok(response),
                Err(error) if attempt < 3 && retryable(&error) => {
                    let delay =
                        Duration::from_millis((250 << attempt) + u64::from(rand::random::<u8>()));
                    tracing::warn!(%path, attempt = attempt + 1, %error, "retrying coordinator request");
                    tokio::time::sleep(delay).await;
                }
                Err(error) => return Err(error),
            }
        }
        unreachable!("the last attempt returns")
    }

    pub async fn register(&self, name: &str, docker_available: bool) -> Result<()> {
        let _: serde_json::Value = self
            .post_retryable(
                "api/v1/runner/register",
                &RegisterRequest {
                    name,
                    os: std::env::consts::OS,
                    arch: std::env::consts::ARCH,
                    version: env!("CARGO_PKG_VERSION"),
                    docker_available,
                },
            )
            .await?;
        Ok(())
    }

    pub async fn touch(&self) -> Result<bool> {
        Ok(self
            .post_json::<_, BoolResponse>("api/v1/runner/touch", &serde_json::json!({}))
            .await?
            .active)
    }

    pub async fn stop(&self) -> Result<()> {
        self.post_empty("api/v1/runner/stop").await
    }

    pub async fn claim(&self) -> Result<Option<Pipeline>> {
        self.claim_with_request(Uuid::new_v4()).await
    }

    pub(crate) async fn claim_with_request(&self, request: Uuid) -> Result<Option<Pipeline>> {
        // A distinct route makes an older coordinator fail with 404 instead of
        // silently ignoring the identity and assigning another job on retry.
        self.post_retryable(
            &format!("api/v1/runner/claim/{request}"),
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn heartbeat(&self, pipeline: Uuid) -> Result<bool> {
        Ok(self
            .post_json::<_, BoolResponse>(
                &format!("api/v1/runner/pipelines/{pipeline}/heartbeat"),
                &serde_json::json!({}),
            )
            .await?
            .active)
    }

    pub async fn finish(&self, pipeline: Uuid, status: &str, error: Option<&str>) -> Result<()> {
        let _: serde_json::Value = self
            .post_retryable(
                &format!("api/v1/runner/pipelines/{pipeline}/finish"),
                &FinishRequest { status, error },
            )
            .await?;
        Ok(())
    }

    pub async fn create_jobs(&self, pipeline: Uuid, config: &PipelineConfig) -> Result<Vec<Job>> {
        self.post_json(&format!("api/v1/runner/pipelines/{pipeline}/jobs"), config)
            .await
    }

    pub async fn job_status(&self, job: Uuid, status: &str, code: Option<i32>) -> Result<()> {
        let _: serde_json::Value = self
            .post_json(
                &format!("api/v1/runner/jobs/{job}/status"),
                &JobStatusRequest { status, code },
            )
            .await?;
        Ok(())
    }

    pub async fn log(
        &self,
        pipeline: Uuid,
        job: Option<Uuid>,
        stream: &str,
        content: &str,
    ) -> Result<()> {
        let _: serde_json::Value = self
            .post_json(
                "api/v1/runner/logs",
                &LogRequest {
                    pipeline,
                    job,
                    stream,
                    content,
                },
            )
            .await?;
        Ok(())
    }

    pub async fn worker_access_token(&self, pipeline: Uuid) -> Result<String> {
        Ok(self
            .post_json::<_, TokenResponse>(
                &format!("api/v1/runner/pipelines/{pipeline}/access-token"),
                &serde_json::json!({}),
            )
            .await?
            .access_token)
    }
}
