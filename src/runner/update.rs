use std::{
    ffi::OsString,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    server::{releases::MAX_RUNNER_BINARY_BYTES, tokens::TokenIssuer},
    version::SemanticVersion,
};

const DEFAULT_UPDATE_INTERVAL_SECONDS: u64 = 5 * 60;

pub struct SelfUpdater {
    client: Client,
    base_url: url::Url,
    pub interval: Duration,
}

#[derive(Debug, PartialEq, Eq)]
pub enum UpdateCheck {
    Current,
    Unavailable,
    Installed(String),
}

#[derive(Debug, Deserialize)]
struct UpdateManifest {
    version: String,
    os: String,
    arch: String,
    sha256: String,
    size: u64,
    download_url: String,
}

impl SelfUpdater {
    pub fn from_environment(issuer: &TokenIssuer) -> Result<Option<Self>> {
        let enabled = match std::env::var("LOREHUB_AUTO_UPDATE") {
            Ok(value) => parse_bool(&value).context("parse LOREHUB_AUTO_UPDATE")?,
            Err(std::env::VarError::NotPresent) => true,
            Err(error) => return Err(error).context("read LOREHUB_AUTO_UPDATE"),
        };
        if !enabled {
            return Ok(None);
        }
        let interval = match std::env::var("LOREHUB_UPDATE_INTERVAL_SECONDS") {
            Ok(value) => value
                .parse::<u64>()
                .context("LOREHUB_UPDATE_INTERVAL_SECONDS must be an integer")?,
            Err(std::env::VarError::NotPresent) => DEFAULT_UPDATE_INTERVAL_SECONDS,
            Err(error) => return Err(error).context("read LOREHUB_UPDATE_INTERVAL_SECONDS"),
        };
        ensure!(
            interval >= 10,
            "LOREHUB_UPDATE_INTERVAL_SECONDS must be at least 10"
        );
        let base_url = std::env::var("LOREHUB_RUNNER_UPDATE_URL")
            .unwrap_or_else(|_| issuer.issuer().to_owned());
        let mut base_url = url::Url::parse(&base_url).context("parse runner update URL")?;
        ensure!(
            matches!(base_url.scheme(), "https" | "http"),
            "runner update URL must use HTTP or HTTPS"
        );
        ensure!(
            base_url.host_str().is_some(),
            "runner update URL requires a host"
        );
        ensure!(
            base_url.scheme() == "https"
                || matches!(base_url.host_str(), Some("127.0.0.1" | "::1" | "localhost")),
            "runner update URL must use HTTPS except on loopback"
        );
        base_url.set_path("/");
        base_url.set_query(None);
        base_url.set_fragment(None);
        Ok(Some(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .context("build runner update HTTP client")?,
            base_url,
            interval: Duration::from_secs(interval),
        }))
    }

    pub async fn check_and_install(
        &self,
        worker_id: Uuid,
        issuer: &TokenIssuer,
    ) -> Result<UpdateCheck> {
        let token = issuer
            .issue_runner_update(&worker_id.to_string())?
            .access_token;
        let manifest_url = self.base_url.join(&format!(
            "api/v1/runner-updates/{}/{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ))?;
        let response = self
            .client
            .get(manifest_url)
            .bearer_auth(&token)
            .send()
            .await
            .context("request runner update manifest")?;
        if matches!(
            response.status(),
            StatusCode::NO_CONTENT | StatusCode::NOT_FOUND
        ) {
            return Ok(UpdateCheck::Unavailable);
        }
        let manifest: UpdateManifest = response
            .error_for_status()
            .context("runner update manifest request failed")?
            .json()
            .await
            .context("decode runner update manifest")?;
        validate_manifest(&manifest)?;
        let available = manifest
            .version
            .parse::<SemanticVersion>()
            .context("parse available runner version")?;
        let current = env!("CARGO_PKG_VERSION").parse::<SemanticVersion>()?;
        if available <= current {
            return Ok(UpdateCheck::Current);
        }

        let download_url = self
            .base_url
            .join(&manifest.download_url)
            .context("resolve runner update download URL")?;
        ensure!(
            download_url.origin() == self.base_url.origin(),
            "runner update download URL must use the coordinator origin"
        );
        let mut response = self
            .client
            .get(download_url)
            .bearer_auth(token)
            .send()
            .await
            .context("download runner update")?
            .error_for_status()
            .context("runner update download failed")?;
        if let Some(length) = response.content_length() {
            ensure!(
                length == manifest.size,
                "runner update content length mismatch"
            );
        }
        let mut binary = Vec::with_capacity(manifest.size as usize);
        while let Some(chunk) = response.chunk().await.context("read runner update")? {
            ensure!(
                binary.len() + chunk.len() <= MAX_RUNNER_BINARY_BYTES as usize,
                "runner update exceeds the size limit"
            );
            binary.extend_from_slice(&chunk);
        }
        ensure!(
            binary.len() as u64 == manifest.size,
            "runner update size mismatch"
        );
        let current_exe = std::env::current_exe().context("find current runner executable")?;
        let expected_sha256 = manifest.sha256.clone();
        tokio::task::spawn_blocking(move || {
            install_update(&current_exe, &binary, &expected_sha256)
        })
        .await
        .context("join runner update installer")??;
        Ok(UpdateCheck::Installed(manifest.version))
    }
}

fn validate_manifest(manifest: &UpdateManifest) -> Result<()> {
    ensure!(
        manifest.os == std::env::consts::OS,
        "runner update OS mismatch"
    );
    ensure!(
        manifest.arch == std::env::consts::ARCH,
        "runner update architecture mismatch"
    );
    ensure!(
        manifest.size > 0 && manifest.size <= MAX_RUNNER_BINARY_BYTES,
        "runner update size is invalid"
    );
    ensure!(
        manifest.sha256.len() == 64 && manifest.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "runner update SHA-256 is invalid"
    );
    Ok(())
}

fn install_update(current_exe: &Path, binary: &[u8], expected_sha256: &str) -> Result<()> {
    let actual_sha256 = format!("{:x}", Sha256::digest(binary));
    ensure!(
        actual_sha256.eq_ignore_ascii_case(expected_sha256),
        "runner update SHA-256 mismatch"
    );
    let parent = current_exe
        .parent()
        .context("runner executable has no parent directory")?;
    #[cfg(windows)]
    let staged = appended_path(current_exe, ".update");
    let temporary = appended_path(current_exe, &format!(".update.tmp-{}", std::process::id()));
    let _ = std::fs::remove_file(&temporary);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .with_context(|| format!("create staged runner update {}", temporary.display()))?;
    file.write_all(binary)?;
    file.sync_all()?;
    let permissions = std::fs::metadata(current_exe)?.permissions();
    std::fs::set_permissions(&temporary, permissions)?;

    #[cfg(unix)]
    {
        std::fs::rename(&temporary, current_exe)
            .with_context(|| format!("replace runner executable {}", current_exe.display()))?;
    }
    #[cfg(windows)]
    {
        if staged.exists() {
            std::fs::remove_file(&staged)?;
        }
        std::fs::rename(&temporary, &staged)
            .with_context(|| format!("stage runner executable {}", staged.display()))?;
    }
    sync_directory(parent)?;
    Ok(())
}

fn appended_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = OsString::from(path.as_os_str());
    value.push(suffix);
    PathBuf::from(value)
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<()> {
    FileSync::sync(path)
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
struct FileSync;

#[cfg(unix)]
impl FileSync {
    fn sync(path: &Path) -> Result<()> {
        std::fs::File::open(path)?.sync_all()?;
        Ok(())
    }
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => anyhow::bail!("expected true or false"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boolean_configuration_is_strict() {
        assert!(parse_bool("YES").unwrap());
        assert!(!parse_bool("off").unwrap());
        assert!(parse_bool("sometimes").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn verified_update_replaces_unix_executable() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let executable = root.path().join("lorehub");
        std::fs::write(&executable, b"old").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o751)).unwrap();
        let digest = format!("{:x}", Sha256::digest(b"new"));
        install_update(&executable, b"new", &digest).unwrap();
        assert_eq!(std::fs::read(&executable).unwrap(), b"new");
        assert_eq!(
            std::fs::metadata(&executable).unwrap().permissions().mode() & 0o777,
            0o751
        );
    }

    #[test]
    fn invalid_update_is_not_written() {
        let root = tempfile::tempdir().unwrap();
        let executable = root.path().join("lorehub");
        std::fs::write(&executable, b"old").unwrap();
        assert!(install_update(&executable, b"new", &"0".repeat(64)).is_err());
        assert_eq!(std::fs::read(&executable).unwrap(), b"old");
    }
}
