use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use axum::{
    Json,
    extract::{Query, State},
    http::{
        HeaderMap, HeaderValue, Method, StatusCode,
        header::{COOKIE, LOCATION, SET_COOKIE},
    },
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use subtle::ConstantTimeEq;
use tokio::sync::RwLock;
use url::Url;
use uuid::Uuid;

pub const ALLOWED_DOMAIN: &str = "zenogrid.co.kr";
const SESSION_COOKIE: &str = "lorehub_session";
const CSRF_COOKIE: &str = "lorehub_csrf";
const OAUTH_STATE_COOKIE: &str = "lorehub_oauth_state";
const CLI_AUTH_COOKIE: &str = "lorehub_cli_auth";
const SESSION_SECONDS: i64 = 8 * 60 * 60;

#[derive(Clone)]
pub struct AuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub public_url: Url,
}

impl AuthConfig {
    pub fn new(client_id: String, client_secret: String, public_url: &str) -> Result<Self> {
        ensure!(
            !client_id.trim().is_empty(),
            "Google client ID cannot be empty"
        );
        ensure!(
            !client_secret.trim().is_empty(),
            "Google client secret cannot be empty"
        );
        let public_url = Url::parse(public_url).context("parse LOREHUB_PUBLIC_URL")?;
        ensure!(
            matches!(public_url.scheme(), "http" | "https"),
            "public URL must use http or https"
        );
        ensure!(
            public_url.host_str().is_some(),
            "public URL requires a host"
        );
        if public_url.scheme() == "http" {
            ensure!(
                matches!(
                    public_url.host_str(),
                    Some("localhost" | "127.0.0.1" | "::1")
                ),
                "non-local public URLs must use https"
            );
        }
        ensure!(
            public_url.query().is_none() && public_url.fragment().is_none(),
            "public URL cannot contain a query or fragment"
        );
        ensure!(public_url.path() == "/", "public URL cannot contain a path");
        Ok(Self {
            client_id,
            client_secret,
            public_url,
        })
    }

    fn callback_url(&self) -> String {
        self.public_url
            .join("auth/google/callback")
            .expect("validated base URL")
            .to_string()
    }

    fn secure_cookies(&self) -> bool {
        self.public_url.scheme() == "https"
    }
}

#[derive(Clone)]
pub struct AuthService {
    pool: PgPool,
    config: Arc<AuthConfig>,
    client: reqwest::Client,
    jwks: Arc<RwLock<Option<CachedJwks>>>,
}

#[derive(Clone)]
struct CachedJwks {
    fetched_at: Instant,
    keys: Vec<Jwk>,
}

#[derive(Debug, Clone, Deserialize)]
struct Jwk {
    kid: String,
    kty: String,
    alg: Option<String>,
    n: String,
    e: String,
}

#[derive(Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    id_token: String,
}

#[derive(Debug, Deserialize)]
struct IdClaims {
    sub: String,
    azp: Option<String>,
    iat: u64,
    email: String,
    email_verified: bool,
    hd: Option<String>,
    nonce: String,
    name: Option<String>,
    picture: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub picture_url: Option<String>,
    pub role: String,
}

#[derive(Clone)]
pub struct AuthSession {
    pub user: User,
    token_hash: Vec<u8>,
}

#[derive(FromRow)]
struct SessionRow {
    id: Uuid,
    email: String,
    name: Option<String>,
    picture_url: Option<String>,
    role: String,
    csrf_hash: Vec<u8>,
}

impl AuthService {
    pub fn new(pool: PgPool, config: AuthConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .https_only(true)
            .timeout(Duration::from_secs(10))
            .user_agent(concat!("lorehub/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            pool,
            config: Arc::new(config),
            client,
            jwks: Arc::new(RwLock::new(None)),
        })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    async fn jwks(&self, force_refresh: bool) -> Result<Vec<Jwk>> {
        if !force_refresh
            && let Some(cache) = self.jwks.read().await.as_ref()
            && cache.fetched_at.elapsed() < Duration::from_secs(3600)
        {
            return Ok(cache.keys.clone());
        }
        let response = self
            .client
            .get("https://www.googleapis.com/oauth2/v3/certs")
            .send()
            .await
            .context("fetch Google signing keys")?
            .error_for_status()
            .context("Google signing-key response")?
            .json::<JwkSet>()
            .await
            .context("decode Google signing keys")?;
        ensure!(!response.keys.is_empty(), "Google returned no signing keys");
        *self.jwks.write().await = Some(CachedJwks {
            fetched_at: Instant::now(),
            keys: response.keys.clone(),
        });
        Ok(response.keys)
    }

    async fn verify_id_token(&self, token: &str, expected_nonce: &str) -> Result<IdClaims> {
        let header = decode_header(token).context("decode Google ID token header")?;
        ensure!(
            header.alg == Algorithm::RS256,
            "unsupported Google ID token algorithm"
        );
        let kid = header.kid.context("Google ID token has no key ID")?;
        for force in [false, true] {
            let keys = self.jwks(force).await?;
            if let Some(jwk) = keys.iter().find(|key| key.kid == kid) {
                ensure!(
                    jwk.kty == "RSA" && jwk.alg.as_deref().is_none_or(|alg| alg == "RS256"),
                    "invalid Google signing key"
                );
                let key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)?;
                let mut validation = Validation::new(Algorithm::RS256);
                validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
                validation.set_audience(&[self.config.client_id.as_str()]);
                validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);
                let claims = decode::<IdClaims>(token, &key, &validation)
                    .context("verify Google ID token")?
                    .claims;
                ensure!(
                    claims
                        .azp
                        .as_deref()
                        .is_none_or(|azp| azp == self.config.client_id),
                    "Google authorized-party mismatch"
                );
                ensure!(
                    bool::from(claims.nonce.as_bytes().ct_eq(expected_nonce.as_bytes())),
                    "Google nonce mismatch"
                );
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .context("system clock is before Unix epoch")?
                    .as_secs();
                ensure!(
                    claims.iat <= now + 60,
                    "Google ID token was issued in the future"
                );
                validate_workspace_identity(&claims)?;
                return Ok(claims);
            }
        }
        bail!("Google signing key not found")
    }
}

fn validate_workspace_identity(claims: &IdClaims) -> Result<()> {
    ensure!(
        !claims.sub.is_empty() && claims.sub.len() <= 255,
        "Google subject is invalid"
    );
    ensure!(claims.email_verified, "Google email is not verified");
    ensure!(
        claims
            .hd
            .as_deref()
            .is_some_and(|domain| domain.eq_ignore_ascii_case(ALLOWED_DOMAIN)),
        "Google Workspace domain is not allowed"
    );
    let (local, domain) = claims
        .email
        .rsplit_once('@')
        .context("Google email is malformed")?;
    ensure!(
        !local.is_empty() && domain.eq_ignore_ascii_case(ALLOWED_DOMAIN),
        "email domain is not allowed"
    );
    Ok(())
}

pub(crate) fn random_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub(crate) fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|part| part.trim().split_once('='))
        .find_map(|(key, value)| (key == name).then(|| value.to_owned()))
}

fn set_cookie(
    name: &str,
    value: &str,
    path: &str,
    max_age: i64,
    secure: bool,
    http_only: bool,
) -> HeaderValue {
    let mut value = format!("{name}={value}; Path={path}; Max-Age={max_age}; SameSite=Lax");
    if secure {
        value.push_str("; Secure");
    }
    if http_only {
        value.push_str("; HttpOnly");
    }
    HeaderValue::from_str(&value).expect("cookie contains generated URL-safe data")
}

pub async fn login(State(auth): State<AuthService>) -> Result<Response, AuthError> {
    let state = random_token();
    let nonce = random_token();
    sqlx::query("DELETE FROM oauth_states WHERE expires_at <= now()")
        .execute(auth.pool())
        .await?;
    sqlx::query("DELETE FROM sessions WHERE expires_at <= now()")
        .execute(auth.pool())
        .await?;
    sqlx::query("INSERT INTO oauth_states (state_hash, nonce, expires_at) VALUES ($1, $2, now() + interval '10 minutes')")
        .bind(token_hash(&state)).bind(&nonce).execute(auth.pool()).await?;
    let mut destination =
        Url::parse("https://accounts.google.com/o/oauth2/v2/auth").expect("static Google URL");
    destination
        .query_pairs_mut()
        .append_pair("client_id", &auth.config.client_id)
        .append_pair("redirect_uri", &auth.config.callback_url())
        .append_pair("response_type", "code")
        .append_pair("scope", "openid email profile")
        .append_pair("state", &state)
        .append_pair("nonce", &nonce)
        .append_pair("hd", ALLOWED_DOMAIN);
    let mut response = StatusCode::SEE_OTHER.into_response();
    response.headers_mut().insert(
        LOCATION,
        HeaderValue::from_str(destination.as_str()).map_err(AuthError::internal)?,
    );
    response.headers_mut().append(
        SET_COOKIE,
        set_cookie(
            OAUTH_STATE_COOKIE,
            &state,
            "/auth/google/callback",
            600,
            auth.config.secure_cookies(),
            true,
        ),
    );
    Ok(response)
}

#[derive(Deserialize)]
pub struct CliLoginQuery {
    session_code: String,
    client_state: String,
}

pub async fn cli_login(
    State(auth): State<AuthService>,
    Query(query): Query<CliLoginQuery>,
) -> Result<Response, AuthError> {
    if query.session_code.len() != 43
        || query.client_state.is_empty()
        || query.client_state.len() > 256
    {
        return Err(AuthError::bad_request("invalid CLI authentication session"));
    }
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM cli_auth_sessions WHERE session_hash=$1 AND client_state=$2 AND user_id IS NULL AND expires_at>now())",
    )
    .bind(token_hash(&query.session_code))
    .bind(&query.client_state)
    .fetch_one(auth.pool())
    .await?;
    if !exists {
        return Err(AuthError::bad_request(
            "CLI authentication session is invalid or expired",
        ));
    }
    let mut response = StatusCode::SEE_OTHER.into_response();
    response
        .headers_mut()
        .insert(LOCATION, HeaderValue::from_static("/auth/google/login"));
    response.headers_mut().append(
        SET_COOKIE,
        set_cookie(
            CLI_AUTH_COOKIE,
            &query.session_code,
            "/auth/google/callback",
            600,
            auth.config.secure_cookies(),
            true,
        ),
    );
    Ok(response)
}

pub async fn cli_complete() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>LoreHub CLI login</title></head><body><main><h1>Lore CLI login complete</h1><p>You can close this window and return to the terminal.</p></main></body></html>",
    )
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

pub async fn callback(
    State(auth): State<AuthService>,
    Query(query): Query<CallbackQuery>,
    headers: HeaderMap,
) -> Result<Response, AuthError> {
    if let Some(error) = query.error {
        return Err(AuthError(
            StatusCode::UNAUTHORIZED,
            format!(
                "Google login failed: {}",
                error.chars().take(100).collect::<String>()
            ),
        ));
    }
    let code = query
        .code
        .context("missing OAuth code")
        .map_err(AuthError::bad_request)?;
    let state = query
        .state
        .context("missing OAuth state")
        .map_err(AuthError::bad_request)?;
    if code.is_empty() || code.len() > 4096 || state.len() != 43 {
        return Err(AuthError::bad_request("invalid OAuth callback parameters"));
    }
    let state_cookie = cookie(&headers, OAUTH_STATE_COOKIE)
        .context("missing OAuth state cookie")
        .map_err(AuthError::unauthorized)?;
    if !bool::from(state.as_bytes().ct_eq(state_cookie.as_bytes())) {
        return Err(AuthError::unauthorized("OAuth state mismatch"));
    }
    let nonce: Option<String> = sqlx::query_scalar(
        "DELETE FROM oauth_states WHERE state_hash = $1 AND expires_at > now() RETURNING nonce",
    )
    .bind(token_hash(&state))
    .fetch_optional(auth.pool())
    .await?;
    let nonce = nonce
        .context("OAuth state is invalid or expired")
        .map_err(AuthError::unauthorized)?;
    let response = auth
        .client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", code.as_str()),
            ("client_id", auth.config.client_id.as_str()),
            ("client_secret", auth.config.client_secret.as_str()),
            ("redirect_uri", auth.config.callback_url().as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(AuthError::upstream)?
        .error_for_status()
        .map_err(AuthError::upstream)?
        .json::<TokenResponse>()
        .await
        .map_err(AuthError::upstream)?;
    let claims = auth
        .verify_id_token(&response.id_token, &nonce)
        .await
        .map_err(|error| AuthError(StatusCode::FORBIDDEN, error.to_string()))?;
    let mut transaction = auth.pool().begin().await?;
    let user_id = Uuid::new_v4();
    let user_id: Uuid = sqlx::query_scalar("INSERT INTO users (id, google_sub, email, name, picture_url) VALUES ($1,$2,$3,$4,$5) ON CONFLICT (google_sub) DO UPDATE SET email = EXCLUDED.email, name = EXCLUDED.name, picture_url = EXCLUDED.picture_url, last_login_at = now() RETURNING id")
        .bind(user_id).bind(&claims.sub).bind(claims.email.to_ascii_lowercase()).bind(&claims.name).bind(&claims.picture)
        .fetch_one(&mut *transaction).await?;
    let session = random_token();
    let csrf = random_token();
    sqlx::query("INSERT INTO sessions (token_hash, user_id, csrf_hash, expires_at) VALUES ($1,$2,$3,now() + interval '8 hours')")
        .bind(token_hash(&session)).bind(user_id).bind(token_hash(&csrf))
        .execute(&mut *transaction).await?;
    let cli_session = cookie(&headers, CLI_AUTH_COOKIE);
    let cli_completed = if let Some(session_code) = cli_session.as_deref() {
        sqlx::query(
            "UPDATE cli_auth_sessions SET user_id=$1, completed_at=now() WHERE session_hash=$2 AND user_id IS NULL AND expires_at>now()",
        )
        .bind(user_id)
        .bind(token_hash(session_code))
        .execute(&mut *transaction)
        .await?
        .rows_affected()
            == 1
    } else {
        false
    };
    transaction.commit().await?;
    let mut response = StatusCode::SEE_OTHER.into_response();
    response.headers_mut().insert(
        LOCATION,
        HeaderValue::from_static(if cli_completed {
            "/auth/cli/complete"
        } else {
            "/"
        }),
    );
    response.headers_mut().append(
        SET_COOKIE,
        set_cookie(
            SESSION_COOKIE,
            &session,
            "/",
            SESSION_SECONDS,
            auth.config.secure_cookies(),
            true,
        ),
    );
    response.headers_mut().append(
        SET_COOKIE,
        set_cookie(
            CSRF_COOKIE,
            &csrf,
            "/",
            SESSION_SECONDS,
            auth.config.secure_cookies(),
            false,
        ),
    );
    response.headers_mut().append(
        SET_COOKIE,
        set_cookie(
            OAUTH_STATE_COOKIE,
            "",
            "/auth/google/callback",
            0,
            auth.config.secure_cookies(),
            true,
        ),
    );
    response.headers_mut().append(
        SET_COOKIE,
        set_cookie(
            CLI_AUTH_COOKIE,
            "",
            "/auth/google/callback",
            0,
            auth.config.secure_cookies(),
            true,
        ),
    );
    Ok(response)
}

pub async fn authenticate(
    auth: &AuthService,
    method: &Method,
    headers: &HeaderMap,
) -> Result<AuthSession, AuthError> {
    let token = cookie(headers, SESSION_COOKIE)
        .context("login required")
        .map_err(AuthError::unauthorized)?;
    let row: Option<SessionRow> = sqlx::query_as("SELECT u.id,u.email,COALESCE(u.display_name,u.name) AS name,u.picture_url,u.role,s.csrf_hash FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.token_hash=$1 AND s.expires_at>now()")
        .bind(token_hash(&token)).fetch_optional(auth.pool()).await?;
    let row = row
        .context("session is invalid or expired")
        .map_err(AuthError::unauthorized)?;
    if !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        let csrf_cookie = cookie(headers, CSRF_COOKIE)
            .context("missing CSRF cookie")
            .map_err(AuthError::forbidden)?;
        let csrf_header = headers
            .get("x-csrf-token")
            .and_then(|value| value.to_str().ok())
            .context("missing CSRF header")
            .map_err(AuthError::forbidden)?;
        if !bool::from(csrf_cookie.as_bytes().ct_eq(csrf_header.as_bytes()))
            || !bool::from(token_hash(csrf_header).ct_eq(&row.csrf_hash))
        {
            return Err(AuthError::forbidden("CSRF token mismatch"));
        }
    }
    Ok(AuthSession {
        user: User {
            id: row.id,
            email: row.email,
            name: row.name,
            picture_url: row.picture_url,
            role: row.role,
        },
        token_hash: token_hash(&token),
    })
}

pub async fn logout(
    State(auth): State<AuthService>,
    axum::extract::Extension(session): axum::extract::Extension<AuthSession>,
) -> Result<Response, AuthError> {
    sqlx::query("DELETE FROM sessions WHERE token_hash=$1")
        .bind(session.token_hash)
        .execute(auth.pool())
        .await?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().append(
        SET_COOKIE,
        set_cookie(
            SESSION_COOKIE,
            "",
            "/",
            0,
            auth.config.secure_cookies(),
            true,
        ),
    );
    response.headers_mut().append(
        SET_COOKIE,
        set_cookie(CSRF_COOKIE, "", "/", 0, auth.config.secure_cookies(), false),
    );
    Ok(response)
}

pub struct AuthError(StatusCode, String);

impl AuthError {
    fn bad_request(error: impl std::fmt::Display) -> Self {
        Self(StatusCode::BAD_REQUEST, error.to_string())
    }
    fn unauthorized(error: impl std::fmt::Display) -> Self {
        Self(StatusCode::UNAUTHORIZED, error.to_string())
    }
    fn forbidden(error: impl std::fmt::Display) -> Self {
        Self(StatusCode::FORBIDDEN, error.to_string())
    }
    fn upstream(error: impl std::fmt::Display) -> Self {
        tracing::warn!(%error, "Google OAuth request failed");
        Self(
            StatusCode::BAD_GATEWAY,
            "Google authentication is temporarily unavailable".into(),
        )
    }
    fn internal(error: impl std::fmt::Display) -> Self {
        tracing::error!(%error, "authentication failed");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "authentication failed".into(),
        )
    }
}

impl From<sqlx::Error> for AuthError {
    fn from(error: sqlx::Error) -> Self {
        Self::internal(error)
    }
}
impl From<anyhow::Error> for AuthError {
    fn from(error: anyhow::Error) -> Self {
        Self::unauthorized(error)
    }
}
impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (self.0, Json(serde_json::json!({"error":self.1}))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims(email: &str, hd: Option<&str>, verified: bool) -> IdClaims {
        IdClaims {
            sub: "google-id".into(),
            azp: None,
            iat: 0,
            email: email.into(),
            email_verified: verified,
            hd: hd.map(str::to_owned),
            nonce: "nonce".into(),
            name: None,
            picture: None,
        }
    }

    #[test]
    fn only_accepts_verified_zenogrid_workspace_identity() {
        assert!(
            validate_workspace_identity(&claims(
                "dev@zenogrid.co.kr",
                Some("zenogrid.co.kr"),
                true
            ))
            .is_ok()
        );
        assert!(
            validate_workspace_identity(&claims("dev@evil.example", Some("zenogrid.co.kr"), true))
                .is_err()
        );
        assert!(
            validate_workspace_identity(&claims("dev@zenogrid.co.kr", Some("evil.example"), true))
                .is_err()
        );
        assert!(
            validate_workspace_identity(&claims(
                "dev@zenogrid.co.kr",
                Some("zenogrid.co.kr"),
                false
            ))
            .is_err()
        );
    }

    #[test]
    fn public_url_requires_https_except_on_loopback() {
        assert!(AuthConfig::new("id".into(), "secret".into(), "http://127.0.0.1:8080").is_ok());
        assert!(
            AuthConfig::new("id".into(), "secret".into(), "https://hub.zenogrid.co.kr").is_ok()
        );
        assert!(
            AuthConfig::new("id".into(), "secret".into(), "http://hub.zenogrid.co.kr").is_err()
        );
        assert!(
            AuthConfig::new(
                "id".into(),
                "secret".into(),
                "https://hub.zenogrid.co.kr/path"
            )
            .is_err()
        );
    }
}
