use std::{
    path::Path,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, ensure};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use super::auth::User;

const USER_TOKEN_SECONDS: u64 = 60 * 60;
const WORKER_TOKEN_SECONDS: u64 = 5 * 60;

#[derive(Clone)]
pub struct TokenIssuer {
    inner: Arc<Inner>,
}

struct Inner {
    key: EncodingKey,
    decoding_key: DecodingKey,
    key_id: String,
    issuer: String,
    audience: String,
    jwks: Value,
}

#[derive(Debug, Serialize)]
pub struct IssuedToken {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: u64,
}

#[derive(Serialize)]
struct Claims {
    sub: String,
    iss: String,
    iat: u64,
    exp: u64,
    aud: Vec<String>,
    name: String,
    preferred_username: String,
    is_service_account: bool,
    resources: Vec<ResourcePermission>,
}

#[derive(Serialize)]
struct ResourcePermission {
    resource_id: String,
    permission: Vec<&'static str>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct VerifiedClaims {
    sub: String,
    iss: String,
    exp: u64,
    aud: Vec<String>,
    resources: Vec<VerifiedResourcePermission>,
}

#[derive(Deserialize)]
struct VerifiedResourcePermission {
    resource_id: String,
}

pub struct VerifiedToken {
    pub subject: String,
    resources: Vec<String>,
}

impl VerifiedToken {
    pub fn has_wildcard(&self) -> bool {
        self.resources.iter().any(|resource| resource == "urc-*")
    }

    pub fn has_exact_resource(&self, resource_id: &str) -> bool {
        self.resources
            .iter()
            .any(|resource| resource == resource_id)
    }
}

fn parse_issuer_url(issuer: &str) -> Result<Url> {
    let issuer_url = Url::parse(issuer).context("parse Lore JWT issuer")?;
    ensure!(
        matches!(issuer_url.scheme(), "https" | "http"),
        "Lore JWT issuer must use http or https"
    );
    ensure!(
        issuer_url.host_str().is_some(),
        "Lore JWT issuer requires a host"
    );
    if issuer_url.scheme() == "http" {
        ensure!(
            matches!(
                issuer_url.host_str(),
                Some("localhost" | "127.0.0.1" | "::1")
            ),
            "non-local Lore JWT issuers must use https"
        );
    }
    ensure!(
        matches!(issuer_url.path(), "" | "/"),
        "Lore JWT issuer cannot contain a path"
    );
    ensure!(
        issuer_url.query().is_none() && issuer_url.fragment().is_none(),
        "Lore JWT issuer cannot contain a query or fragment"
    );
    Ok(issuer_url)
}

impl TokenIssuer {
    pub fn from_files(
        private_key_path: impl AsRef<Path>,
        jwks_path: impl AsRef<Path>,
        issuer: &str,
        audience: &str,
    ) -> Result<Self> {
        parse_issuer_url(issuer)?;
        ensure!(
            !audience.trim().is_empty(),
            "Lore JWT audience cannot be empty"
        );

        let private_key = std::fs::read(private_key_path.as_ref()).with_context(|| {
            format!(
                "read Lore JWT private key {}",
                private_key_path.as_ref().display()
            )
        })?;
        let key = EncodingKey::from_rsa_pem(&private_key).context("parse Lore JWT private key")?;
        let jwks: Value = serde_json::from_slice(
            &std::fs::read(jwks_path.as_ref())
                .with_context(|| format!("read Lore JWKS {}", jwks_path.as_ref().display()))?,
        )
        .context("parse Lore JWKS")?;
        let key_id = jwks["keys"]
            .as_array()
            .and_then(|keys| keys.first())
            .and_then(|key| key["kid"].as_str())
            .context("Lore JWKS requires a key ID")?
            .to_owned();
        let jwk = &jwks["keys"][0];
        let decoding_key = DecodingKey::from_rsa_components(
            jwk["n"]
                .as_str()
                .context("Lore JWKS requires RSA modulus")?,
            jwk["e"]
                .as_str()
                .context("Lore JWKS requires RSA exponent")?,
        )
        .context("parse Lore JWKS public key")?;

        Ok(Self {
            inner: Arc::new(Inner {
                key,
                decoding_key,
                key_id,
                issuer: issuer.trim_end_matches('/').to_owned(),
                audience: audience.to_owned(),
                jwks,
            }),
        })
    }

    pub fn issue_user(&self, user: &User, resources: Vec<String>) -> Result<IssuedToken> {
        let name = user.name.clone().unwrap_or_else(|| user.email.clone());
        self.issue(
            user.id.to_string(),
            name,
            user.email.clone(),
            USER_TOKEN_SECONDS,
            resources,
        )
    }

    pub fn issue_repository_creation(&self, user: &User) -> Result<IssuedToken> {
        let name = user.name.clone().unwrap_or_else(|| user.email.clone());
        self.issue(
            user.id.to_string(),
            name,
            user.email.clone(),
            USER_TOKEN_SECONDS,
            vec!["urc-*".to_owned()],
        )
    }

    pub fn issue_worker(&self, worker_id: &str, resource_id: String) -> Result<IssuedToken> {
        self.issue(
            format!("lorehub-worker:{worker_id}"),
            "LoreHub worker".to_owned(),
            "lorehub-worker@zenogrid.co.kr".to_owned(),
            WORKER_TOKEN_SECONDS,
            vec![resource_id],
        )
    }

    pub fn issue_runner_update(&self, worker_id: &str) -> Result<IssuedToken> {
        self.issue(
            format!("lorehub-worker:{worker_id}"),
            "LoreHub worker".to_owned(),
            "lorehub-worker@zenogrid.co.kr".to_owned(),
            WORKER_TOKEN_SECONDS,
            Vec::new(),
        )
    }

    pub fn issuer(&self) -> &str {
        &self.inner.issuer
    }

    pub fn jwks(&self) -> Value {
        self.inner.jwks.clone()
    }

    pub fn discovery(&self) -> Value {
        serde_json::json!({
            "issuer": self.inner.issuer,
            "jwks_uri": format!("{}/.well-known/jwks.json", self.inner.issuer),
            "id_token_signing_alg_values_supported": ["RS256"]
        })
    }

    pub fn verify_access_token(&self, token: &str) -> Result<VerifiedToken> {
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&self.inner.issuer]);
        validation.set_audience(&[&self.inner.audience]);
        let claims = decode::<VerifiedClaims>(token, &self.inner.decoding_key, &validation)
            .context("verify Lore access token")?
            .claims;
        Ok(VerifiedToken {
            subject: claims.sub,
            resources: claims
                .resources
                .into_iter()
                .map(|resource| resource.resource_id)
                .collect(),
        })
    }

    fn issue(
        &self,
        subject: String,
        name: String,
        username: String,
        lifetime: u64,
        resources: Vec<String>,
    ) -> Result<IssuedToken> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let claims = Claims {
            sub: subject,
            iss: self.inner.issuer.clone(),
            iat: now,
            exp: now + lifetime,
            aud: vec![self.inner.audience.clone()],
            name,
            preferred_username: username,
            is_service_account: false,
            resources: resources
                .into_iter()
                .map(|resource_id| ResourcePermission {
                    resource_id,
                    permission: Vec::new(),
                })
                .collect(),
        };
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.inner.key_id.clone());
        let access_token =
            encode(&header, &claims, &self.inner.key).context("sign Lore access token")?;
        Ok(IssuedToken {
            access_token,
            token_type: "Bearer",
            expires_in: lifetime,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issuer_requires_https_except_on_loopback() {
        assert!(parse_issuer_url("http://127.0.0.1:8080").is_ok());
        assert!(parse_issuer_url("http://localhost:8080").is_ok());
        assert!(parse_issuer_url("https://hub.zenogrid.co.kr").is_ok());
        assert!(parse_issuer_url("http://hub.zenogrid.co.kr").is_err());
        assert!(parse_issuer_url("https://hub.zenogrid.co.kr/issuer").is_err());
    }
}
