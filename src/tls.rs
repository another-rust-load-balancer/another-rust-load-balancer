use std::io::{self, ErrorKind::InvalidData};
use std::{fs::File, io::BufReader, path::Path, sync::Arc};
use tokio_rustls::rustls::{
  internal::pemfile::{certs, rsa_private_keys},
  sign::{CertifiedKey, RSASigningKey},
  Certificate, PrivateKey, ResolvesServerCertUsingSNI,
};

pub fn add_certificate(
  cert_resolver: &mut ResolvesServerCertUsingSNI,
  dns_name: &str,
  certificate_path: &Path,
  private_key_path: &Path,
) -> Result<(), io::Error> {
  let certificates = load_certs(certificate_path)?;
  let private_key = load_key(private_key_path)?;
  let private_key = RSASigningKey::new(&private_key).map_err(|_| io::Error::new(InvalidData, "invalid rsa key"))?;
  let certificate_key = CertifiedKey::new(certificates, Arc::new(Box::new(private_key)));
  cert_resolver
      .add(dns_name, certificate_key)
      .map_err(|e| io::Error::new(InvalidData, e))
}

fn load_certs(path: &Path) -> io::Result<Vec<Certificate>> {
  let file = File::open(path)?;
  let mut reader = BufReader::new(file);
  certs(&mut reader).map_err(|_| io::Error::new(InvalidData, "invalid cert"))
}

fn load_key(path: &Path) -> io::Result<PrivateKey> {
  let mut keys = load_keys(path)?;
  Ok(keys.remove(0))
}

fn load_keys(path: &Path) -> io::Result<Vec<PrivateKey>> {
  let file = File::open(path)?;
  let mut reader = BufReader::new(file);
  rsa_private_keys(&mut reader).map_err(|_| io::Error::new(InvalidData, "invalid key"))
}
