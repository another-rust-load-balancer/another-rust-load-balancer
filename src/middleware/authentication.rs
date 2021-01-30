use super::{Context, Middleware};
use async_trait::async_trait;
use http_auth_basic::Credentials;
use hyper::{
  header::{AUTHORIZATION, WWW_AUTHENTICATE},
  Body, HeaderMap, Request, Response, StatusCode,
};
use ldap3::LdapConnAsync;

#[derive(Debug)]
pub struct Authentication {}

#[async_trait]
impl Middleware for Authentication {
  async fn modify_request(
    &self,
    request: Request<Body>,
    _context: &Context<'_>,
  ) -> Result<Request<Body>, Response<Body>> {
    if user_authentication(request.headers()).await.is_some() {
      Ok(request)
    } else {
      Err(response_unauthorized())
    }
  }
}

async fn user_authentication(headers: &HeaderMap) -> Option<()> {
  let auth_data = headers.get(AUTHORIZATION)?.to_str().ok()?;
  let credentials = Credentials::from_header(auth_data.to_string()).ok()?;
  check_user_credentials(&credentials.user_id, &credentials.password).await
}

async fn check_user_credentials(user: &str, password: &str) -> Option<()> {
  let (conn, mut ldap) = LdapConnAsync::new("ldap://localhost:389").await.ok()?;
  ldap3::drive!(conn);
  let bind_user = format!("uid={},ou=users,dc=arlb,dc=de", user);
  let bind_result = ldap.simple_bind(&bind_user, password).await.ok()?;
  if bind_result.success().is_ok() {
    Some(())
  } else {
    None
  }
}

fn response_unauthorized() -> Response<Body> {
  let response_builder = Response::builder();
  let response = response_builder
    .header(
      WWW_AUTHENTICATE,
      "Basic realm=\"Another Rust Load Balancer requires authentication\"",
    )
    .status(StatusCode::UNAUTHORIZED)
    .body(Body::from("401 - unauthorized"))
    .unwrap();
  response
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_user_authentication_no_header() {
    // given:
    let headers = HeaderMap::new();

    // when:
    let auth_data = user_authentication(&headers).await;

    // then:
    assert!(auth_data.is_none());
  }

  #[tokio::test]
  async fn test_user_authentication_wrong_protocol() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, "Bearer fpKL54jvWmEGVoRdCNjG".parse().unwrap());

    // when:
    let auth_data = user_authentication(&headers).await;

    // then:
    assert!(auth_data.is_none());
  }

  #[tokio::test]
  async fn test_user_authentication_basic_authorized() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, "Basic dHlyaW9uOmZvbw==".parse().unwrap());

    // when:
    let auth_data = user_authentication(&headers).await;

    // then:
    assert!(auth_data.is_some());
  }

  #[tokio::test]
  async fn test_user_authentication_basic_unauthorized() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, "Basic dHlyaW9uOmZvbwP==".parse().unwrap());

    // when:
    let auth_data = user_authentication(&headers).await;

    // then:
    assert!(auth_data.is_none());
  }
}
