use anyhow::{Context, Result, bail};
use reqwest::{Client, Method, Response};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

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
        bail!("coordinator request failed ({status}): {message}")
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
        Self::checked(
            self.request(Method::POST, path)
                .await?
                .json(body)
                .send()
                .await?,
        )
        .await?
        .json()
        .await
        .context("decode coordinator response")
    }

    pub async fn register(&self, name: &str, docker_available: bool) -> Result<()> {
        let _: serde_json::Value = self
            .post_json(
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
        self.post_json("api/v1/runner/claim", &serde_json::json!({}))
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
            .post_json(
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
