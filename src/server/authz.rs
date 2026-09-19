use std::{
    net::SocketAddr,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status, metadata::MetadataMap, transport::Server};

use super::{
    auth::{User, random_token, token_hash},
    tokens::{TokenIssuer, VerifiedToken},
};

pub mod epic_urc {
    tonic::include_proto!("epic_urc");
}

pub mod ucs {
    pub mod auth {
        tonic::include_proto!("ucs.auth");
    }
}

#[derive(Clone)]
struct AuthzService {
    pool: PgPool,
    tokens: TokenIssuer,
    public_url: String,
}

impl AuthzService {
    fn authenticate(&self, metadata: &MetadataMap) -> Result<VerifiedToken, Status> {
        let authorization = metadata
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or_else(|| Status::unauthenticated("authorization header required"))?;
        self.tokens
            .verify_access_token(authorization)
            .map_err(|_| Status::unauthenticated("invalid or expired access token"))
    }

    async fn can_access(
        &self,
        identity: &VerifiedToken,
        resource_id: &str,
    ) -> Result<bool, Status> {
        super::repository_access::can_access(&self.pool, &identity.subject, resource_id)
            .await
            .map_err(|_| Status::internal("check Lore resource access"))
    }
}

#[tonic::async_trait]
impl epic_urc::urc_auth_api_server::UrcAuthApi for AuthzService {
    async fn health_check(
        &self,
        _request: Request<epic_urc::HealthCheckRequest>,
    ) -> Result<Response<epic_urc::HealthCheckResponse>, Status> {
        Ok(Response::new(epic_urc::HealthCheckResponse {
            status: "ok".to_owned(),
        }))
    }

    async fn check_user_permission(
        &self,
        request: Request<epic_urc::CheckUserPermissionRequest>,
    ) -> Result<Response<epic_urc::CheckUserPermissionResponse>, Status> {
        let identity = self.authenticate(request.metadata())?;
        let mut allowed = Vec::new();
        let mut denied = Vec::new();
        for resource_id in request.into_inner().resource_id {
            let authorized = if identity.subject.starts_with("lorehub-worker:") {
                identity.has_exact_resource(&resource_id)
            } else {
                identity.has_exact_resource(&resource_id)
                    && self.can_access(&identity, &resource_id).await?
            };
            let permission = epic_urc::ResourcePermission {
                resource_id,
                permission: Vec::new(),
            };
            if authorized {
                allowed.push(permission);
            } else {
                denied.push(permission);
            }
        }
        Ok(Response::new(epic_urc::CheckUserPermissionResponse {
            allowed_resource_permission: allowed,
            denied_resource_permission: denied,
        }))
    }

    async fn lookup_user_permissions(
        &self,
        request: Request<epic_urc::LookupUserPermissionsRequest>,
    ) -> Result<Response<epic_urc::LookupUserPermissionsResponse>, Status> {
        let identity = self.authenticate(request.metadata())?;
        let filter = request.into_inner().resource_filter;
        let ids: Vec<String> =
            if filter == "urc" && !identity.subject.starts_with("lorehub-worker:") {
                super::repository_access::resource_ids(&self.pool, &identity.subject)
                    .await
                    .map_err(|_| Status::internal("load Lore resources"))?
            } else {
                Vec::new()
            };
        Ok(Response::new(epic_urc::LookupUserPermissionsResponse {
            resource_permission: ids
                .into_iter()
                .filter(|id| identity.has_exact_resource(id) || identity.has_wildcard())
                .map(|resource_id| epic_urc::ResourcePermission {
                    resource_id,
                    permission: Vec::new(),
                })
                .collect(),
            next_page_token: None,
        }))
    }

    async fn start_auth_session(
        &self,
        request: Request<epic_urc::StartAuthSessionRequest>,
    ) -> Result<Response<epic_urc::StartAuthSessionResponse>, Status> {
        let client_state = request.into_inner().client_state;
        if client_state.is_empty() || client_state.len() > 256 {
            return Err(Status::invalid_argument("invalid client state"));
        }
        sqlx::query("DELETE FROM cli_auth_sessions WHERE expires_at<=now()")
            .execute(&self.pool)
            .await
            .map_err(|_| Status::internal("clean expired CLI authentication sessions"))?;
        let session_code = random_token();
        sqlx::query("INSERT INTO cli_auth_sessions (session_hash,client_state,expires_at) VALUES ($1,$2,now() + interval '10 minutes')")
            .bind(token_hash(&session_code))
            .bind(&client_state)
            .execute(&self.pool)
            .await
            .map_err(|_| Status::internal("create CLI authentication session"))?;
        let mut login_url = url::Url::parse(&self.public_url)
            .and_then(|url| url.join("auth/cli"))
            .map_err(|_| Status::internal("build CLI authentication URL"))?;
        login_url
            .query_pairs_mut()
            .append_pair("session_code", &session_code)
            .append_pair("client_state", &client_state);
        Ok(Response::new(epic_urc::StartAuthSessionResponse {
            session_code,
            login_url: login_url.to_string(),
        }))
    }
    async fn get_auth_session(
        &self,
        request: Request<epic_urc::GetAuthSessionRequest>,
    ) -> Result<Response<epic_urc::GetAuthSessionResponse>, Status> {
        let request = request.into_inner();
        if request.session_code.len() != 43
            || request.client_state.is_empty()
            || request.client_state.len() > 256
        {
            return Err(Status::invalid_argument(
                "invalid CLI authentication session",
            ));
        }
        let user_id: Option<Option<uuid::Uuid>> = sqlx::query_scalar(
            "SELECT user_id FROM cli_auth_sessions WHERE session_hash=$1 AND client_state=$2 AND expires_at>now()",
        )
        .bind(token_hash(&request.session_code))
        .bind(&request.client_state)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| Status::internal("load CLI authentication session"))?;
        let Some(user_id) = user_id else {
            return Err(Status::not_found("CLI authentication session expired"));
        };
        let Some(user_id) = user_id else {
            return Ok(Response::new(epic_urc::GetAuthSessionResponse {
                user_token: None,
            }));
        };
        let user: User =
            sqlx::query_as("SELECT id,email,name,picture_url,role FROM users WHERE id=$1")
                .bind(user_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|_| Status::internal("load CLI user"))?;
        let resources = super::repository_access::resource_ids(&self.pool, &user.id.to_string())
            .await
            .map_err(|_| Status::internal("load CLI user resources"))?;
        let issued = self
            .tokens
            .issue_user(&user, resources)
            .map_err(|_| Status::internal("issue CLI user token"))?;
        sqlx::query("DELETE FROM cli_auth_sessions WHERE session_hash=$1")
            .bind(token_hash(&request.session_code))
            .execute(&self.pool)
            .await
            .map_err(|_| Status::internal("complete CLI authentication session"))?;
        let expires_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Status::internal("system clock is before Unix epoch"))?
            .as_millis()
            .saturating_add(u128::from(issued.expires_in) * 1000)
            .min(i64::MAX as u128) as i64;
        Ok(Response::new(epic_urc::GetAuthSessionResponse {
            user_token: Some(epic_urc::UserToken {
                user_token: issued.access_token,
                expires_at,
                user_id: user.id.to_string(),
                user_name: user.name.unwrap_or(user.email),
            }),
        }))
    }
    async fn refresh_auth_session(
        &self,
        _: Request<epic_urc::RefreshAuthSessionRequest>,
    ) -> Result<Response<epic_urc::RefreshAuthSessionResponse>, Status> {
        Err(Status::unimplemented("issue a new LoreHub token"))
    }
    async fn verify_user(
        &self,
        _: Request<epic_urc::VerifyUserRequest>,
    ) -> Result<Response<epic_urc::VerifyUserResponse>, Status> {
        Err(Status::unimplemented("not required"))
    }
    async fn exchange_external_token_for_user_token(
        &self,
        _: Request<epic_urc::ExchangeExternalTokenForUserTokenRequest>,
    ) -> Result<Response<epic_urc::ExchangeExternalTokenForUserTokenResponse>, Status> {
        Err(Status::unimplemented("use LoreHub token directly"))
    }
    async fn exchange_api_key_for_user_token(
        &self,
        _: Request<epic_urc::ExchangeApiKeyForUserTokenRequest>,
    ) -> Result<Response<epic_urc::ExchangeApiKeyForUserTokenResponse>, Status> {
        Err(Status::unimplemented("API key login is disabled"))
    }
    async fn exchange_user_token_for_multiresource_token(
        &self,
        request: Request<epic_urc::ExchangeUserTokenForMultiresourceTokenRequest>,
    ) -> Result<Response<epic_urc::ExchangeUserTokenForMultiresourceTokenResponse>, Status> {
        let identity = self.authenticate(request.metadata())?;
        let resources = request.into_inner().resource_id;
        if resources.is_empty() {
            return Err(Status::invalid_argument(
                "at least one resource is required",
            ));
        }
        for resource in &resources {
            if !resource.starts_with("urc-") || !self.can_access(&identity, resource).await? {
                return Err(Status::permission_denied("repository access denied"));
            }
        }
        let user: User =
            sqlx::query_as("SELECT id,email,name,picture_url,role FROM users WHERE id::text=$1")
                .bind(&identity.subject)
                .fetch_optional(&self.pool)
                .await
                .map_err(|_| Status::internal("load token user"))?
                .ok_or_else(|| Status::permission_denied("user account not found"))?;
        let issued = self
            .tokens
            .issue_user(&user, resources)
            .map_err(|_| Status::internal("issue repository token"))?;
        let expires_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Status::internal("system clock is before Unix epoch"))?
            .as_millis()
            .saturating_add(u128::from(issued.expires_in) * 1000)
            .min(i64::MAX as u128) as i64;
        Ok(Response::new(
            epic_urc::ExchangeUserTokenForMultiresourceTokenResponse {
                token: Some(epic_urc::UserToken {
                    user_token: issued.access_token,
                    expires_at,
                    user_id: user.id.to_string(),
                    user_name: user.name.unwrap_or(user.email),
                }),
            },
        ))
    }
    async fn get_user_info(
        &self,
        _: Request<epic_urc::GetUserInfoRequest>,
    ) -> Result<Response<epic_urc::GetUserInfoResponse>, Status> {
        Err(Status::unimplemented("not required"))
    }
    async fn get_user_id(
        &self,
        _: Request<epic_urc::GetUserIdRequest>,
    ) -> Result<Response<epic_urc::GetUserIdResponse>, Status> {
        Err(Status::unimplemented("not required"))
    }
    async fn get_provider_user_id(
        &self,
        _: Request<epic_urc::GetProviderUserIdRequest>,
    ) -> Result<Response<epic_urc::GetProviderUserIdResponse>, Status> {
        Err(Status::unimplemented("not required"))
    }
}

#[tonic::async_trait]
impl ucs::auth::rebac_api_server::RebacApi for AuthzService {
    async fn create_resource(
        &self,
        request: Request<ucs::auth::CreateResourceRequest>,
    ) -> Result<Response<ucs::auth::CreateResourceResponse>, Status> {
        let identity = self.authenticate(request.metadata())?;
        let resource = request.into_inner();
        if !identity.has_wildcard() || !resource.resource_id.starts_with("urc-") {
            return Err(Status::permission_denied("repository creation denied"));
        }
        let inserted = sqlx::query(
            "INSERT INTO lore_resources (resource_id, name, owner_subject) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
        )
        .bind(&resource.resource_id)
        .bind(&resource.resource_name)
        .bind(&identity.subject)
        .execute(&self.pool)
        .await
        .map_err(|_| Status::internal("store Lore resource"))?;
        if inserted.rows_affected() == 0 {
            return Err(Status::already_exists("resource already exists"));
        }
        Ok(Response::new(ucs::auth::CreateResourceResponse {}))
    }

    async fn delete_resource(
        &self,
        request: Request<ucs::auth::DeleteResourceRequest>,
    ) -> Result<Response<ucs::auth::DeleteResourceResponse>, Status> {
        let identity = self.authenticate(request.metadata())?;
        let resource = request.into_inner();
        if !identity.has_exact_resource(&resource.resource_id)
            || !super::repository_access::delete(
                &self.pool,
                &identity.subject,
                &resource.resource_id,
            )
            .await
            .map_err(|_| Status::internal("delete Lore resource"))?
        {
            return Err(Status::permission_denied(
                "repository owner or administrator required",
            ));
        }
        Ok(Response::new(ucs::auth::DeleteResourceResponse {}))
    }
}

pub async fn serve(
    pool: PgPool,
    tokens: TokenIssuer,
    public_url: String,
    bind: SocketAddr,
    shutdown: CancellationToken,
) -> Result<()> {
    let service = AuthzService {
        pool,
        tokens,
        public_url,
    };
    tracing::info!(%bind, "Lore authorization service listening");
    Server::builder()
        .add_service(epic_urc::urc_auth_api_server::UrcAuthApiServer::new(
            service.clone(),
        ))
        .add_service(ucs::auth::rebac_api_server::RebacApiServer::new(service))
        .serve_with_shutdown(bind, shutdown.cancelled_owned())
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use epic_urc::urc_auth_api_server::UrcAuthApi;
    use ucs::auth::rebac_api_server::RebacApi;
    use uuid::Uuid;

    fn request<T>(body: T, token: &str) -> Request<T> {
        let mut request = Request::new(body);
        request
            .metadata_mut()
            .insert("authorization", format!("Bearer {token}").parse().unwrap());
        request
    }

    #[sqlx::test]
    #[ignore = "requires DATABASE_URL pointing to a disposable PostgreSQL instance"]
    async fn administrators_access_all_resources_and_demotion_revokes_existing_tokens(
        pool: PgPool,
    ) {
        let tokens = TokenIssuer::from_files(
            "tests/fixtures/test-private.pem",
            "tests/fixtures/test-jwks.json",
            "http://127.0.0.1:8080",
            "zenogrid.co.kr",
        )
        .unwrap();
        let admin = Uuid::new_v4();
        let owner = Uuid::new_v4();
        for id in [admin, owner] {
            sqlx::query("INSERT INTO users(id,google_sub,email) VALUES($1,$2,$3)")
                .bind(id)
                .bind(id.to_string())
                .bind(format!("{id}@zenogrid.co.kr"))
                .execute(&pool)
                .await
                .unwrap();
        }
        for (resource, subject) in [("urc-admin", admin), ("urc-project", owner)] {
            sqlx::query(
                "INSERT INTO lore_resources(resource_id,name,owner_subject) VALUES($1,$1,$2)",
            )
            .bind(resource)
            .bind(subject.to_string())
            .execute(&pool)
            .await
            .unwrap();
        }
        let user: User =
            sqlx::query_as("SELECT id,email,name,picture_url,role FROM users WHERE id=$1")
                .bind(admin)
                .fetch_one(&pool)
                .await
                .unwrap();
        let ids = super::super::repository_access::resource_ids(&pool, &admin.to_string())
            .await
            .unwrap();
        assert_eq!(ids, ["urc-admin", "urc-project"]);
        assert_eq!(
            super::super::repository_access::resource_ids(&pool, &owner.to_string())
                .await
                .unwrap(),
            ["urc-project"]
        );
        let token = tokens.issue_user(&user, ids).unwrap().access_token;
        let service = AuthzService {
            pool: pool.clone(),
            tokens,
            public_url: "http://127.0.0.1:8080".into(),
        };
        let lookup = || {
            request(
                epic_urc::LookupUserPermissionsRequest {
                    resource_filter: "urc".into(),
                    ..Default::default()
                },
                &token,
            )
        };
        let check = || {
            request(
                epic_urc::CheckUserPermissionRequest {
                    resource_id: vec![
                        "urc-admin".into(),
                        "urc-project".into(),
                        "urc-missing".into(),
                    ],
                    ..Default::default()
                },
                &token,
            )
        };
        let exchange = || {
            request(
                epic_urc::ExchangeUserTokenForMultiresourceTokenRequest {
                    resource_id: vec!["urc-project".into()],
                    ..Default::default()
                },
                &token,
            )
        };
        assert_eq!(
            service
                .lookup_user_permissions(lookup())
                .await
                .unwrap()
                .into_inner()
                .resource_permission
                .len(),
            2
        );
        let permissions = service
            .check_user_permission(check())
            .await
            .unwrap()
            .into_inner();
        assert_eq!(permissions.allowed_resource_permission.len(), 2);
        assert_eq!(
            permissions.denied_resource_permission[0].resource_id,
            "urc-missing"
        );
        let exchanged = service
            .exchange_user_token_for_multiresource_token(exchange())
            .await
            .unwrap()
            .into_inner()
            .token
            .unwrap();
        assert!(
            service
                .tokens
                .verify_access_token(&exchanged.user_token)
                .unwrap()
                .has_exact_resource("urc-project")
        );

        // CLI login must receive the same complete resource list as web tokens.
        let session_code = random_token();
        sqlx::query("INSERT INTO cli_auth_sessions(session_hash,client_state,user_id,expires_at) VALUES($1,'test-state',$2,now()+interval '1 minute')")
            .bind(token_hash(&session_code)).bind(admin).execute(&pool).await.unwrap();
        let cli = service
            .get_auth_session(Request::new(epic_urc::GetAuthSessionRequest {
                session_code,
                client_state: "test-state".into(),
            }))
            .await
            .unwrap()
            .into_inner()
            .user_token
            .unwrap();
        assert!(
            service
                .tokens
                .verify_access_token(&cli.user_token)
                .unwrap()
                .has_exact_resource("urc-project")
        );

        // Worker tokens stay limited to the one repository in their claim.
        let worker = service
            .tokens
            .issue_worker("test", "urc-project".into())
            .unwrap()
            .access_token;
        let worker_permissions = service
            .check_user_permission(request(
                epic_urc::CheckUserPermissionRequest {
                    resource_id: vec!["urc-project".into(), "urc-admin".into()],
                    ..Default::default()
                },
                &worker,
            ))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(worker_permissions.allowed_resource_permission.len(), 1);
        assert_eq!(
            worker_permissions.denied_resource_permission[0].resource_id,
            "urc-admin"
        );
        assert!(
            service
                .delete_resource(request(
                    ucs::auth::DeleteResourceRequest {
                        resource_id: "urc-project".into()
                    },
                    &worker
                ))
                .await
                .is_err()
        );

        sqlx::query("UPDATE users SET role='user' WHERE id=$1")
            .bind(admin)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            service
                .lookup_user_permissions(lookup())
                .await
                .unwrap()
                .into_inner()
                .resource_permission
                .len(),
            1
        );
        assert_eq!(
            service
                .check_user_permission(check())
                .await
                .unwrap()
                .into_inner()
                .allowed_resource_permission
                .len(),
            1
        );
        assert_eq!(
            service
                .exchange_user_token_for_multiresource_token(exchange())
                .await
                .unwrap_err()
                .code(),
            tonic::Code::PermissionDenied
        );
        assert!(
            service
                .delete_resource(request(
                    ucs::auth::DeleteResourceRequest {
                        resource_id: "urc-project".into()
                    },
                    &token
                ))
                .await
                .is_err()
        );
        assert!(
            super::super::repository_access::by_name(&pool, &admin.to_string(), "urc-project")
                .await
                .unwrap()
                .is_none()
        );

        // Repository grants follow live group membership even for already-issued tokens.
        let group = Uuid::new_v4();
        sqlx::query("INSERT INTO account_groups(id,name,owner_id) VALUES($1,'Project team',$2)")
            .bind(group)
            .bind(owner)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO account_group_members(group_id,user_id) VALUES($1,$2)")
            .bind(group)
            .bind(admin)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO repository_account_group_access(resource_id,group_id) VALUES('urc-project',$1)")
            .bind(group)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            super::super::repository_access::resource_ids(&pool, &admin.to_string())
                .await
                .unwrap(),
            ["urc-admin", "urc-project"]
        );
        assert_eq!(
            service
                .check_user_permission(check())
                .await
                .unwrap()
                .into_inner()
                .allowed_resource_permission
                .len(),
            2
        );
        assert!(
            service
                .delete_resource(request(
                    ucs::auth::DeleteResourceRequest {
                        resource_id: "urc-project".into()
                    },
                    &token
                ))
                .await
                .is_err()
        );
        sqlx::query("DELETE FROM account_group_members WHERE group_id=$1 AND user_id=$2")
            .bind(group)
            .bind(admin)
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            !super::super::repository_access::can_access(&pool, &admin.to_string(), "urc-project")
                .await
                .unwrap()
        );

        sqlx::query("UPDATE users SET role='admin' WHERE id=$1")
            .bind(admin)
            .execute(&pool)
            .await
            .unwrap();
        service
            .delete_resource(request(
                ucs::auth::DeleteResourceRequest {
                    resource_id: "urc-project".into(),
                },
                &token,
            ))
            .await
            .unwrap();
        assert!(
            !super::super::repository_access::can_access(&pool, &admin.to_string(), "urc-project")
                .await
                .unwrap()
        );
    }
}
