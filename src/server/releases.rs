use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::version::SemanticVersion;

pub const MAX_RUNNER_BINARY_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Default)]
pub struct RunnerReleases {
    releases: Arc<HashMap<(String, String), RunnerRelease>>,
}

#[derive(Clone)]
pub struct RunnerRelease {
    pub manifest: RunnerReleaseManifest,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
pub struct RunnerReleaseManifest {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub sha256: String,
    pub size: u64,
    pub download_url: String,
}

impl RunnerReleases {
    pub fn load(directory: &Path) -> Result<Self> {
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!(path = %directory.display(), "runner release directory does not exist; self-update is unavailable");
                return Ok(Self::default());
            }
            Err(error) => return Err(error).context("read runner release directory"),
        };
        let mut releases: HashMap<(String, String), (SemanticVersion, RunnerRelease)> =
            HashMap::new();
        for entry in entries {
            let entry = entry.context("read runner release directory entry")?;
            let path = entry.path();
            if !entry.file_type()?.is_file() {
                continue;
            }
            let Some((os, arch, version)) = parse_release_filename(&entry.file_name())? else {
                continue;
            };
            let metadata = entry.metadata()?;
            ensure!(
                metadata.len() <= MAX_RUNNER_BINARY_BYTES,
                "runner release {} exceeds the size limit",
                path.display()
            );
            let key = (os.clone(), arch.clone());
            let manifest = RunnerReleaseManifest {
                version: version.to_string(),
                os: os.clone(),
                arch: arch.clone(),
                sha256: sha256_file(&path)?,
                size: metadata.len(),
                download_url: format!("/api/v1/runner-updates/{os}/{arch}/binary"),
            };
            let release = RunnerRelease { manifest, path };
            match releases.get(&key) {
                Some((current, _)) if current >= &version => {}
                _ => {
                    releases.insert(key, (version, release));
                }
            }
        }
        tracing::info!(count = releases.len(), path = %directory.display(), "runner releases loaded");
        Ok(Self {
            releases: Arc::new(
                releases
                    .into_iter()
                    .map(|(key, (_, release))| (key, release))
                    .collect(),
            ),
        })
    }

    pub fn get(&self, os: &str, arch: &str) -> Option<RunnerRelease> {
        self.releases
            .get(&(os.to_owned(), arch.to_owned()))
            .cloned()
    }
}

fn parse_release_filename(
    name: &std::ffi::OsStr,
) -> Result<Option<(String, String, SemanticVersion)>> {
    let Some(name) = name.to_str() else {
        return Ok(None);
    };
    let name = name.strip_suffix(".exe").unwrap_or(name);
    if !name.starts_with("lorehub-runner-") {
        return Ok(None);
    }
    let mut parsed = None;
    for os in ["linux", "macos", "windows"] {
        for arch in ["x86_64", "aarch64"] {
            let prefix = format!("lorehub-runner-{os}-{arch}-v");
            if let Some(version) = name.strip_prefix(&prefix) {
                parsed = Some((os, arch, version));
            }
        }
    }
    let Some((os, arch, version)) = parsed else {
        bail!("invalid runner release filename {name}");
    };
    let version = version
        .parse::<SemanticVersion>()
        .with_context(|| format!("parse runner release version in {name}"))?;
    Ok(Some((os.to_owned(), arch.to_owned(), version)))
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file =
        File::open(path).with_context(|| format!("open runner release {}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_release_is_selected_and_hashed() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("lorehub-runner-macos-aarch64-v0.1.0"),
            b"old",
        )
        .unwrap();
        std::fs::write(
            root.path().join("lorehub-runner-macos-aarch64-v0.2.0"),
            b"new",
        )
        .unwrap();
        std::fs::write(root.path().join("README.md"), b"ignored").unwrap();

        let releases = RunnerReleases::load(root.path()).unwrap();
        let release = releases.get("macos", "aarch64").unwrap();
        assert_eq!(release.manifest.version, "0.2.0");
        assert_eq!(release.manifest.size, 3);
        assert_eq!(
            release.manifest.sha256,
            "11507a0e2f5e69d5dfa40a62a1bd7b6ee57e6bcd85c67c9b8431b36fff21c437"
        );
    }

    #[test]
    fn malformed_release_name_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("lorehub-runner-linux-x86_64-latest"), b"x").unwrap();
        assert!(RunnerReleases::load(root.path()).is_err());
    }
}
