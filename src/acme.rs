use acme_lib::{Error, Directory, DirectoryUrl, Certificate};
use acme_lib::persist::FilePersist;
use acme_lib::create_p384_key;
use hyper::{Request, Body, Response, StatusCode};
use acme_lib::order::NewOrder;
use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use std::ops::Add;
use chrono::{Duration, Utc, DateTime};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_util::either::Either::{Left, Right};
use tokio_util::either::Either;
use crate::server::{bad_request, not_found};
use std::sync::{Arc, Mutex};

static CERTS: &str = "certs.toml";

mod date_serializer {
  use chrono::{Utc, DateTime};
  use serde::{Deserialize, Deserializer, Serializer, Serialize};
  use serde::de::Error;
  use std::str::FromStr;

  pub fn serialize<S: Serializer>(time: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error> {
    time.to_string().serialize(serializer)
  }

  pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<DateTime<Utc>, D::Error> {
    let time: String = Deserialize::deserialize(deserializer)?;
    Ok(DateTime::<Utc>::from_str(&time).map_err(D::Error::custom)?)
  }

}

#[derive(Clone)]
struct OpenChallenge {
  token: String,
  proof: String
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CertInfo {
  primary_name: String,
  alt_names: Vec<String>,
  #[serde(with = "date_serializer")]
  expiration_date: DateTime<Utc>,
}

pub struct AcmeHandler {
  persist_dir: String,
  email: String,
  challenges: Arc<Mutex<Vec<OpenChallenge>>>,
  certs: Arc<Mutex<Vec<CertInfo>>>,
}

impl CertInfo {
  pub fn has_expired(&self) -> bool {
    let now = Utc::now();
    self.expiration_date.gt(&now)
  }
}

impl AcmeHandler {
  pub fn new(persist_dir: String, email: String) -> AcmeHandler {
    let certs_path: PathBuf = [&persist_dir, CERTS].iter().collect();
    let certs_path = certs_path.as_path();
    let certs = std::fs::read_to_string(certs_path)
      .map(|file_content| {
        toml::from_str(&file_content).unwrap_or(Vec::new())
      })
      .unwrap_or(Vec::new());

    AcmeHandler {
      persist_dir,
      email,
      challenges: Arc::new(Mutex::new(Vec::new())),
      certs: Arc::new(Mutex::new(certs))
    }
  }

  fn add_challenge(&self, token: &str, proof: String) {
    let challenge = OpenChallenge { token: token.to_string(), proof };
    let mut challenges = self.challenges.lock().unwrap();
    challenges.push(challenge);
  }

  fn get_and_remove_challenge_for_token(&self, token: &str) -> Option<OpenChallenge> {
    let mut challenges = self.challenges.lock().unwrap();
    challenges.iter().position(|c| c.token == token)
      .map(|index| challenges.remove(index))
  }

  fn start_challenge_handler(ord_new: NewOrder<FilePersist>,
                             cs: UnboundedSender<Either<(String, String), Result<Certificate, Error>>>) {
    // TODO maybe add own implementation of the acme lib so we can use an async fn instead of a thread
    fn generate_and_validate_challenge(
      mut ord_new: NewOrder<FilePersist>,
      cs: &UnboundedSender<Either<(String, String), Result<Certificate, Error>>>
    ) -> Result<Certificate, Error> {
      loop {
        if let Some(ord_csr) = ord_new.confirm_validations() {
          let pkey_pri = create_p384_key();
          let ord_cert = ord_csr.finalize_pkey(pkey_pri, 5000)?;
          return ord_cert.download_and_save_cert();
        }

        let auths = ord_new.authorizations()?;
        let chall = auths[0].http_challenge();
        let token = chall.http_token().to_string();
        let proof = chall.http_proof();
        cs.send(Left((token, proof)))
          .map_err(|e| acme_lib::Error::Other(e.to_string()))?;

        chall.validate(5000)?;
        ord_new.refresh()?;
      }
    }

    std::thread::spawn(move || {
      let result = generate_and_validate_challenge(ord_new, &cs);
      let _ = cs.send(Right(result));
    });
  }

  pub async fn initiate_challenge(&mut self, primary_name: &str, alt_names: &[&str]) -> Result<CertInfo, Error> {
    let persist = FilePersist::new(&self.persist_dir);
    let dir = Directory::from_url(persist, DirectoryUrl::LetsEncryptStaging)?;
    let acc = dir.account(&self.email)?;
    let ord_new = acc.new_order(primary_name, alt_names)?;

    let (cs, mut cr) = unbounded_channel();
    AcmeHandler::start_challenge_handler(ord_new, cs);

    let cert = {
      let mut result = cr.recv().await.unwrap();
      loop {
        match result {
          Left((token, proof)) => {
            self.add_challenge(&token, proof);
            result = cr.recv().await.unwrap();
            let _ = self.get_and_remove_challenge_for_token(&token);
          },
          Right(cert) => break cert?
        }
      }
    };

    let now = Utc::now();
    let expiration_date = now.add(Duration::days(cert.valid_days_left()));
    let cert_info = CertInfo {
      primary_name: primary_name.to_string(),
      alt_names: alt_names.iter().map(|s| s.to_string()).collect(),
      expiration_date
    };

    let mut certs = self.certs.lock().unwrap();
    certs.push(cert_info.clone());
    let certs_path: PathBuf = [&self.persist_dir, CERTS].iter().collect();
    let certs_str = toml::to_string(&*certs)
      .map_err(|e| acme_lib::Error::Other(e.to_string()))?;
    std::fs::write(certs_path, certs_str)
      .map_err(|e| acme_lib::Error::Other(e.to_string()))?;
    Ok(cert_info)
  }

  pub fn is_challenge(&self, request: &Request<Body>) -> bool {
    request.uri().path().starts_with("/.well-known/acme-challenge/")
  }

  pub async fn respond_to_challenge(&self, request: Request<Body>) -> Response<Body> {
    if !self.is_challenge(&request) {
      return bad_request();
    }

    request.uri().path().split("/").last()
      .map(|token| self.get_and_remove_challenge_for_token(token)
        .map(|challenge| Response::builder()
          .status(StatusCode::OK)
          .body(Body::from(challenge.proof))
          .unwrap())
        .unwrap_or(not_found()))
      .unwrap_or(bad_request())
  }
}



#[cfg(test)]
mod tests {
  use super::*;
  use hyper::body;

  #[test]
  fn test_is_challenge_only_matches_acme_challenge() {
    let valid_req = Request::builder()
      .uri("https://test.de/.well-known/acme-challenge/sdkpgjJASF12")
      .body(Body::empty())
      .unwrap();
    let invalid_req = Request::builder()
      .uri("https://test.de/admin/users")
      .body(Body::empty())
      .unwrap();
    let handler = AcmeHandler::new(".".to_string(), "test@test.de".to_string());

    assert!(handler.is_challenge(&valid_req));
    assert!(!handler.is_challenge(&invalid_req))
  }

  #[test]
  fn test_add_challenge_correctly_adds_challenge() {
    let handler = AcmeHandler::new(".".to_string(), "test@test.de".to_string());
    handler.add_challenge("sdkpgjJASF12", "abc".to_string());

    let challenges = handler.challenges.lock().unwrap();
    assert_eq!(challenges[0].token, "sdkpgjJASF12");
    assert_eq!(challenges[0].proof, "abc");
  }

  #[test]
  fn test_get_and_remove_challenge_correctly_removes_challenge() {
    let handler = AcmeHandler::new(".".to_string(), "test@test.de".to_string());
    handler.add_challenge("sdkpgjJASF12", "abc".to_string());
    let challenge = handler.get_and_remove_challenge_for_token("abc");
    assert!(challenge.is_none());
    let challenge = handler.get_and_remove_challenge_for_token("sdkpgjJASF12");
    assert!(challenge.is_some());
    let challenge = challenge.unwrap();
    assert_eq!(challenge.token, "sdkpgjJASF12");
    assert_eq!(challenge.proof, "abc");
  }

  #[test]
  fn test_respond_to_challenge_extracts_correct_token() {
    let req = Request::builder()
      .uri("https://test.de/.well-known/acme-challenge/sdkpgjJASF12")
      .body(Body::empty())
      .unwrap();

    let handler = AcmeHandler::new(".".to_string(), "test@test.de".to_string());
    handler.add_challenge("sdkpgjJASF12", "abc".to_string());
    let response = tokio_test::block_on(handler.respond_to_challenge(req));
    let status = response.status();
    let body = response.into_body();
    let body = tokio_test::block_on(body::to_bytes(body))
      .map(|bytes| String::from_utf8(bytes.to_vec()).unwrap_or(String::new()))
      .unwrap_or(String::new());

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "abc");
  }
}