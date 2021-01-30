use std::io::{self, ErrorKind::InvalidData};
use std::{fs::File, io::BufReader, path::Path, sync::Arc};
use tokio_rustls::rustls::{
  internal::pemfile::{certs, rsa_private_keys},
  sign::{CertifiedKey, RSASigningKey},
  Certificate, PrivateKey, ResolvesServerCertUsingSNI,
};

pub fn add_certificate<P1, P2>(
  cert_resolver: &mut ResolvesServerCertUsingSNI,
  dns_name: &str,
  certificate_path: P1,
  private_key_path: P2,
) -> Result<(), io::Error>
where
  P1: AsRef<Path>,
  P2: AsRef<Path>,
{
  let certificates = load_certs(certificate_path)?;
  let private_key = load_key(private_key_path)?;
  let private_key = RSASigningKey::new(&private_key).map_err(|_| io::Error::new(InvalidData, "invalid rsa key"))?;
  let certificate_key = CertifiedKey::new(certificates, Arc::new(Box::new(private_key)));
  cert_resolver
    .add(dns_name, certificate_key)
    .map_err(|e| io::Error::new(InvalidData, e))
}

fn load_certs<P>(path: P) -> io::Result<Vec<Certificate>>
where
  P: AsRef<Path>,
{
  let file = File::open(path)?;
  let mut reader = BufReader::new(file);
  certs(&mut reader).map_err(|_| io::Error::new(InvalidData, "invalid cert"))
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
  let file = File::open(path)?;
  let mut reader = BufReader::new(file);
  rsa_private_keys(&mut reader).map_err(|_| io::Error::new(InvalidData, "invalid key"))
}
