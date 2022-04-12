use arc_swap::access::Access;
use rustls_pemfile::{certs, rsa_private_keys};
use std::{
  collections::HashMap,
  fs::File,
  io::{self, BufReader, ErrorKind::InvalidData},
  path::Path,
  sync::Arc,
};
use tokio_rustls::{
  rustls::{
    server::{ClientHello, ResolvesServerCert},
    sign::{CertifiedKey, RsaSigningKey},
    Certificate, PrivateKey,
  },
  webpki::{DnsName, DnsNameRef},
};

pub fn certified_key_from_acme_certificate(certificate: acme_lib::Certificate) -> Result<CertifiedKey, io::Error> {
  let certificates: Vec<Certificate> = certs(&mut certificate.certificate().as_bytes())
    .map_err(|_| io::Error::new(InvalidData, "Invalid certificate"))?
    .into_iter()
    .map(|arg| Certificate(arg))
    .collect();

  let private_key = PrivateKey(certificate.private_key_der());
  let private_key = RsaSigningKey::new(&private_key).map_err(|_| io::Error::new(InvalidData, "Invalid RSA key"))?;

  Ok(CertifiedKey::new(certificates, Arc::new(private_key)))
}

pub fn load_certified_key<P1, P2>(certificate_path: P1, private_key_path: P2) -> Result<CertifiedKey, io::Error>
where
  P1: AsRef<Path>,
  P2: AsRef<Path>,
{
  let certificates = load_certs(certificate_path)?;
  let private_key = load_key(&private_key_path)?;
  let private_key = RsaSigningKey::new(&private_key).map_err(|_| {
    io::Error::new(
      InvalidData,
      format!("Invalid RSA key in '{}'", private_key_path.as_ref().display()),
    )
  })?;
  Ok(CertifiedKey::new(certificates, Arc::new(private_key)))
}

fn load_certs<P>(path: P) -> io::Result<Vec<Certificate>>
where
  P: AsRef<Path>,
{
  let file = File::open(&path).map_err(|e| {
    io::Error::new(
      e.kind(),
      format!("Could not open '{}' due to: {}", path.as_ref().display(), e),
    )
  })?;
  let mut reader = BufReader::new(file);
  certs(&mut reader)
    .map_err(|_| {
      io::Error::new(
        InvalidData,
        format!("Invalid certificate in '{}'", path.as_ref().display()),
      )
    })
    .and_then(|r| Ok(r.into_iter().map(|item| Certificate(item)).collect()))
}

fn load_key<P>(path: P) -> io::Result<PrivateKey>
where
  P: AsRef<Path>,
{
  let mut keys = load_keys(path)?;
  Ok(keys.remove(0))
}

fn load_keys<P>(path: P) -> io::Result<Vec<PrivateKey>>
where
  P: AsRef<Path>,
{
  let file = File::open(&path)?;
  let mut reader = BufReader::new(file);
  rsa_private_keys(&mut reader)
    .map_err(|_| io::Error::new(InvalidData, format!("Invalid RSA key in '{}'", path.as_ref().display())))
    .and_then(|r| Ok(r.into_iter().map(|item| PrivateKey(item)).collect()))
}

pub struct ReconfigurableCertificateResolver<A>
where
  A: Access<HashMap<DnsName, CertifiedKey>>,
{
  certificates: A,
}

impl<A> ReconfigurableCertificateResolver<A>
where
  A: Access<HashMap<DnsName, CertifiedKey>>,
{
  pub fn new(certificates: A) -> ReconfigurableCertificateResolver<A> {
    ReconfigurableCertificateResolver { certificates }
  }
}

impl<A> ResolvesServerCert for ReconfigurableCertificateResolver<A>
where
  A: Access<HashMap<DnsName, CertifiedKey>> + Send + Sync,
{
  fn resolve(&self, client_hello: ClientHello) -> Option<Arc<CertifiedKey>> {
    client_hello.server_name().and_then(|name|{
      DnsNameRef::try_from_ascii_str(name).map_or_else(
        |_err| None,
        |dns| {
          self
            .certificates
            .load()
            .get(&dns.to_owned())
            .cloned()
            .and_then(|k| Some(Arc::new(k)))
        },
      )
    })
  }
}
