use super::{Context, Middleware};
use async_trait::async_trait;
use http_auth_basic::Credentials;
use hyper::{
  header::{AUTHORIZATION, WWW_AUTHENTICATE},
  Body, HeaderMap, Request, Response, StatusCode,
};
use ldap3::{LdapConnAsync, LdapError, Scope, SearchEntry};
use log::error;

#[derive(Debug)]
pub struct Authentication {
  pub ldap_address: String,
  pub user_directory: String,
  pub rdn_identifier: String,
  pub recursive: bool,
}

// HTTP Basic Auth according to RFC 7617
#[async_trait]
impl Middleware for Authentication {
  async fn modify_request(
    &self,
    request: Request<Body>,
    _context: &Context<'_>,
  ) -> Result<Request<Body>, Response<Body>> {
    if self.user_authentication(request.headers()).await.is_some() {
      Ok(request)
    } else {
      Err(response_unauthorized())
    }
  }
}

impl Authentication {
  async fn user_authentication(&self, headers: &HeaderMap) -> Option<()> {
    let auth_data = headers.get(AUTHORIZATION)?.to_str().ok()?;
    let credentials = Credentials::from_header(auth_data.to_string()).ok()?;
    let auth_result = self
      .check_user_credentials(&credentials.user_id, &credentials.password)
      .await
      .map_err(|e| error!("{}", e))
      .ok()?;
    if auth_result {
      Some(())
    } else {
      None
    }
  }

  async fn check_user_credentials(&self, user: &str, password: &str) -> Result<bool, LdapError> {
    let (conn, mut ldap) = LdapConnAsync::new(&self.ldap_address).await?;
    ldap3::drive!(conn);
    let scope = if self.recursive {
      Scope::Subtree
    } else {
      Scope::OneLevel
    };
    let filter = format!("({}={})", self.rdn_identifier, user);
    let (result_entry, _) = ldap
      .search(&self.user_directory, scope, &filter, vec!["1.1"])
      .await?
      .success()?;

    for entry in result_entry {
      let sn = SearchEntry::construct(entry);
      let bind_user = ldap.simple_bind(&sn.dn, password).await?;
      if bind_user.success().is_ok() {
        return Ok(true);
      }
    }
    Ok(false)
  }
}

fn response_unauthorized() -> Response<Body> {
  Response::builder()
    .header(
      WWW_AUTHENTICATE,
      "Basic realm=\"Another Rust Load Balancer requires authentication\"",
    )
    .status(StatusCode::UNAUTHORIZED)
    .body(Body::empty())
    .unwrap()
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
    let ldap_address = "ldap://172.28.1.7:1389".to_string();
    let user_directory = "ou=users,dc=example,dc=org".to_string();
    let rdn_identifier = "cn".to_string();
    let recursive = false;

    let auth = Authentication {
      ldap_address,
      user_directory,
      rdn_identifier,
      recursive,
    };
    // when:
    let auth_data = auth.user_authentication(&headers).await;

    // then:
    assert!(auth_data.is_none());
  }

  #[tokio::test]
  #[ignore]
  async fn test_user_authentication_wrong_protocol() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, "Bearer fpKL54jvWmEGVoRdCNjG".parse().unwrap());
    let ldap_address = "ldap://172.28.1.7:1389".to_string();
    let user_directory = "ou=users,dc=example,dc=org".to_string();
    let rdn_identifier = "cn".to_string();
    let recursive = false;

    let auth = Authentication {
      ldap_address,
      user_directory,
      rdn_identifier,
      recursive,
    };
    // when:
    let auth_data = auth.user_authentication(&headers).await;

    // then:
    assert!(auth_data.is_none());
  }

  #[tokio::test]
  #[ignore]
  async fn test_user_authentication_basic_authorized() {
    // given:
    let mut headers = HeaderMap::new();
    //authorized user tyrion:foo
    headers.insert(AUTHORIZATION, "Basic dHlyaW9uOmZvbw==".parse().unwrap());
    let ldap_address = "ldap://172.28.1.7:1389".to_string();
    let user_directory = "ou=users,dc=example,dc=org".to_string();
    let rdn_identifier = "cn".to_string();
    let recursive = false;

    let auth = Authentication {
      ldap_address,
      user_directory,
      rdn_identifier,
      recursive,
    };
    // when:
    let auth_data = auth.user_authentication(&headers).await;

    // then:
    assert!(auth_data.is_some());
  }

  #[tokio::test]
  #[ignore]
  async fn test_user_authentication_basic_unauthorized() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, "Basic dHlyaW9uOmFicg==".parse().unwrap());
    let ldap_address = "ldap://172.28.1.7:1389".to_string();
    let user_directory = "ou=users,dc=example,dc=org".to_string();
    let rdn_identifier = "cn".to_string();
    let recursive = false;

    let auth = Authentication {
      ldap_address,
      user_directory,
      rdn_identifier,
      recursive,
    };
    // when:
    let auth_data = auth.user_authentication(&headers).await;

    // then:
    assert!(auth_data.is_none());
  }

  #[tokio::test]
  #[ignore]
  async fn test_user_authentication_invalid_user_directory() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, "Basic dHlyaW9uOmZvbw==".parse().unwrap());
    let ldap_address = "ldap://172.28.1.7:1389".to_string();
    let user_directory = "ou=org,dc=example,dc=org".to_string();
    let rdn_identifier = "cn".to_string();
    let recursive = false;

    let auth = Authentication {
      ldap_address,
      user_directory,
      rdn_identifier,
      recursive,
    };
    // when:
    let auth_data = auth.user_authentication(&headers).await;

    // then:
    assert!(auth_data.is_none());
  }
  #[tokio::test]
  #[ignore]
  async fn test_user_authentication_invalid_ldap_address() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, "Basic dHlyaW9uOmZvbw==".parse().unwrap());
    let ldap_address = "ldap://172.28.1.7:1386".to_string();
    let user_directory = "ou=users,dc=example,dc=org".to_string();
    let rdn_identifier = "cn".to_string();
    let recursive = false;

    let auth = Authentication {
      ldap_address,
      user_directory,
      rdn_identifier,
      recursive,
    };
    // when:
    let auth_data = auth.user_authentication(&headers).await;

    // then:
    assert!(auth_data.is_none());
  }

  #[tokio::test]
  #[ignore]
  async fn test_user_authentication_recursive() {
    // given:
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, "Basic dHlyaW9uOmZvbw==".parse().unwrap());
    let ldap_address = "ldap://172.28.1.7:1389".to_string();
    let user_directory = "dc=example,dc=org".to_string();
    let rdn_identifier = "cn".to_string();
    let recursive = true;

    let auth = Authentication {
      ldap_address,
      user_directory,
      rdn_identifier,
      recursive,
    };
    // when:
    let auth_data = auth.user_authentication(&headers).await;

    // then:
    assert!(auth_data.is_some());
  }
}
