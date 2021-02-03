use super::{Context, Middleware};
use async_trait::async_trait;
use http_auth_basic::Credentials;
use hyper::{
  header::{AUTHORIZATION, WWW_AUTHENTICATE},
  Body, HeaderMap, Request, Response, StatusCode,
};
use ldap3::LdapConnAsync;
use log::warn;

#[derive(Debug)]
pub struct Authentication {
  pub ldap_address: String,
  pub user_directory: String,
}

// HTTP Basic Auth according to RFC 7617
#[async_trait]
impl Middleware for Authentication {
  async fn modify_request(
    &self,
    request: Request<Body>,
    _context: &Context<'_>,
  ) -> Result<Request<Body>, Response<Body>> {
    if user_authentication(request.headers(), &self.ldap_address, &self.user_directory)
      .await
      .is_some()
    {
      Ok(request)
    } else {
      Err(response_unauthorized())
    }
  }
}

async fn user_authentication(headers: &HeaderMap, ldap_address: &str, user_directory: &str) -> Option<()> {
  let auth_data = headers.get(AUTHORIZATION)?.to_str().ok()?;
  let credentials = Credentials::from_header(auth_data.to_string()).ok()?;
  check_user_credentials(
    ldap_address,
    user_directory,
    &credentials.user_id,
    &credentials.password,
  )
  .await
}

async fn check_user_credentials(ldap_address: &str, user_directory: &str, user: &str, password: &str) -> Option<()> {
  let connection = LdapConnAsync::new(ldap_address).await;
  if let Ok((conn, mut ldap)) = connection {
    ldap3::drive!(conn);
    let bind_user = user_directory.replace("{}", user);
    let bind_result = ldap.simple_bind(&bind_user, password).await.ok()?;
    if bind_result.success().is_ok() {
      Some(())
    } else {
      None
    }
  } else {
    warn!("Could not connect to LDAP");
    None
  }
}

// TODO status 401 or 407?
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
  #[ignore]
  //run with cargo test middleware::authentication::tests::test_user_authentication_no_header -- --ignored
  async fn test_user_authentication_no_header() {
    // given:
    let headers = HeaderMap::new();
    let ldap_address = "ldap://172.28.1.7:1389";
    let user_directory = "cn={},ou=users,dc=example,dc=org";

    // when:
    let auth_data = user_authentication(&headers, ldap_address, user_directory).await;

    // then:
    assert!(auth_data.is_none());
  }

  #[tokio::test]
  #[ignore]
  async fn test_user_authentication_wrong_protocol() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, "Bearer fpKL54jvWmEGVoRdCNjG".parse().unwrap());
    let ldap_address = "ldap://172.28.1.7:1389";
    let user_directory = "cn={},ou=users,dc=example,dc=org";

    // when:
    let auth_data = user_authentication(&headers, ldap_address, user_directory).await;

    // then:
    assert!(auth_data.is_none());
  }

  #[tokio::test]
  #[ignore]
  async fn test_user_authentication_basic_authorized() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, "Basic dHlyaW9uOmZvbw==".parse().unwrap());
    let ldap_address = "ldap://172.28.1.7:1389";
    let user_directory = "cn={},ou=users,dc=example,dc=org";

    // when:
    let auth_data = user_authentication(&headers, ldap_address, user_directory).await;

    // then:
    assert!(auth_data.is_some());
  }

  #[tokio::test]
  #[ignore]
  async fn test_user_authentication_basic_unauthorized() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, "Basic dHlyaW9uOmFicg==".parse().unwrap());
    let ldap_address = "ldap://172.28.1.7:1389";
    let user_directory = "cn={},ou=users,dc=example,dc=org";

    // when:
    let auth_data = user_authentication(&headers, ldap_address, user_directory).await;

    // then:
    assert!(auth_data.is_none());
  }
}
