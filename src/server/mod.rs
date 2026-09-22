//! HTTP application lifecycle and API routes.
use std::{future::IntoFuture, net::SocketAddr, time::Duration};

use anyhow::Result;
use tokio_util::sync::CancellationToken;

use crate::ci::db;
pub mod api;
pub mod auth;
pub mod authz;
mod execution;
mod links;
mod management;
mod operations;
mod pipeline_access;
pub mod releases;
pub mod repositories;
pub(crate) mod repository_access;
pub mod tokens;
pub mod triggers;
pub mod web;

pub struct ServeConfig {
    pub google_client_id: String,
    pub google_client_secret: String,
    pub public_url: String,
    pub lore_bin: String,
    pub lore_server_url: String,
    pub lore_server_public_url: Option<String>,
    pub lore_local_server_url: Option<String>,
    pub lore_local_server_public_url: Option<String>,
    pub lore_jwt_private_key: std::path::PathBuf,
    pub lore_jwt_jwks: std::path::PathBuf,
    pub runner_releases_dir: std::path::PathBuf,
    pub auth_bind: SocketAddr,
    pub bind: SocketAddr,
}

pub async fn serve(
    database_url: &str,
    config: ServeConfig,
    shutdown: CancellationToken,
) -> Result<()> {
    let pool = db::connect(database_url).await?;
    let auth_config = auth::AuthConfig::new(
        config.google_client_id,
        config.google_client_secret,
        &config.public_url,
    )?;
    let auth_public_url = config.public_url.clone();
    let auth = auth::AuthService::new(pool.clone(), auth_config)?;
    let tokens = tokens::TokenIssuer::from_files(
        config.lore_jwt_private_key,
        config.lore_jwt_jwks,
        &config.public_url,
        auth::ALLOWED_DOMAIN,
    )?;
    let public_server_url = config
        .lore_server_public_url
        .as_deref()
        .unwrap_or(&config.lore_server_url)
        .to_owned();
    let mut repositories = repositories::RepositoryService::new(
        config.lore_bin.clone(),
        &config.lore_server_url,
        &public_server_url,
    )?;
    if let Some(local_server_url) = config.lore_local_server_url.as_deref() {
        let local_public_url = config
            .lore_local_server_public_url
            .as_deref()
            .unwrap_or(local_server_url);
        repositories = repositories.with_local_backend(local_server_url, local_public_url)?;
    }
    let runner_releases = releases::RunnerReleases::load(&config.runner_releases_dir)?;
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    let triggers = tokio::spawn(triggers::run(
        pool.clone(),
        config.lore_bin,
        repositories.clone(),
        tokens.clone(),
        shutdown.clone(),
    ));
    let reaper = tokio::spawn({
        let pool = pool.clone();
        let shutdown = shutdown.clone();
        async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => break,
                    _ = interval.tick() => if let Err(error) = db::reap(&pool).await { tracing::error!(%error, "lease reaper failed"); }
                }
            }
        }
    });
    tracing::info!(bind = %config.bind, "coordinator listening");
    let mut http = Box::pin(
        axum::serve(
            listener,
            api::router_with_releases(
                pool.clone(),
                auth,
                repositories,
                Some(tokens.clone()),
                runner_releases,
            ),
        )
        .with_graceful_shutdown(shutdown.clone().cancelled_owned())
        .into_future(),
    );
    let mut authz = Box::pin(authz::serve(
        pool,
        tokens,
        auth_public_url,
        config.auth_bind,
        shutdown.clone(),
    ));
    let result: Result<()> = tokio::select! {
        result = &mut http => {
            result?;
            shutdown.cancel();
            authz.await
        }
        result = &mut authz => {
            result?;
            shutdown.cancel();
            http.await?;
            Ok(())
        }
    };
    shutdown.cancel();
    reaper.await?;
    triggers.await?;
    result?;
    Ok(())
}
