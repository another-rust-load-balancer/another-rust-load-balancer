use bytes::BytesMut;
use log::{trace, LevelFilter};
use log4rs::{
  append::console::ConsoleAppender,
  config::{Appender, Root},
  Config,
};
use std::io::{self, ErrorKind::InvalidData};
use std::{fs::File, io::BufReader, path::Path, sync::Arc, sync::Mutex};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tokio::{
  io::{split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
  net::{TcpListener, TcpStream},
  try_join,
};
use tokio_rustls::{
  rustls::{
    internal::pemfile::{certs, rsa_private_keys},
    sign::{CertifiedKey, RSASigningKey},
    Certificate, NoClientAuth, PrivateKey, ResolvesServerCertUsingSNI, ServerConfig,
  },
  TlsAcceptor,
};

const LOCAL_HTTP_ADDRESS: &str = "0.0.0.0:3000";
const LOCAL_HTTPS_ADDRESS: &str = "0.0.0.0:3001";
// for now, provide Backend IPs as global fixed sized slice
static REMOTE_ADDRESSES : [&'static str; 3] = ["172.28.1.1:80","172.28.1.2:80","172.28.1.3:80"];
// possible choices: Static Random, Round Robin, Http Host
static LB_MEHOD: &str = "Static Random";

#[tokio::main]
pub async fn main() -> Result<(), io::Error> {
  let stdout = ConsoleAppender::builder().build();
  let config = Config::builder()
    .appender(Appender::builder().build("stdout", Box::new(stdout)))
    .build(Root::builder().appender("stdout").build(LevelFilter::Trace))
    .unwrap();
  log4rs::init_config(config).expect("Logging should not fail");

  let round_robin_counter  = Arc::new(Mutex::new(0));
  let rrc_handle1 = round_robin_counter.clone();
  let rrc_handle2 = round_robin_counter.clone();

  try_join!(listen_for_http_request(rrc_handle1), listen_for_https_request(rrc_handle2))?;

  Ok(())
}

async fn listen_for_http_request(rrc: Arc<Mutex<u32>>) -> Result<(), io::Error> {
  let listener = TcpListener::bind(LOCAL_HTTP_ADDRESS).await?;
  loop {
    let (stream, _) = listener.accept().await?;
    let rrc = rrc.clone();
    let remote_addr = get_remote_addr(&stream, rrc).await;
    tokio::spawn(process_stream(stream, remote_addr));
  }
}

async fn listen_for_https_request(rrc: Arc<Mutex<u32>>) -> Result<(), io::Error> {
  let mut tls_config = ServerConfig::new(NoClientAuth::new());
  let mut cert_resolver = ResolvesServerCertUsingSNI::new();
  add_certificate(
    &mut cert_resolver,
    "localhost",
    Path::new("x509/localhost.cer"),
    Path::new("x509/localhost.key"),
  )?;
  add_certificate(
    &mut cert_resolver,
    "www.arlb.de",
    Path::new("x509/www.arlb.de.cer"),
    Path::new("x509/www.arlb.de.key"),
  )?;
  tls_config.cert_resolver = Arc::new(cert_resolver);
  let tls_acceptor = TlsAcceptor::from(Arc::new(tls_config));

  let listener = TcpListener::bind(LOCAL_HTTPS_ADDRESS).await?;

  loop {
    let (stream, _) = listener.accept().await?;
    let tls_acceptor = tls_acceptor.clone();
    let rrc = rrc.clone();
    tokio::spawn(process_https_stream(stream, tls_acceptor, rrc));
  }
}

fn add_certificate(
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

async fn process_https_stream(stream: TcpStream, tls_acceptor: TlsAcceptor, rrc: Arc<Mutex<u32>>) -> Result<(), io::Error> {
  let tls_stream = tls_acceptor.accept(stream).await?;
  let remote_addr = get_remote_addr(tls_stream.get_ref().0, rrc).await;
  process_stream(tls_stream, remote_addr).await
}

async fn process_stream<S: AsyncRead + AsyncWrite>(client: S, remote_addr: String) -> Result<(), io::Error> {
  let (mut client_read, mut client_write) = split(client);

  let server = TcpStream::connect(remote_addr).await?;
  let (mut server_read, mut server_write) = split(server);

  try_join!(
    pipe_stream(&mut client_read, &mut server_write),
    pipe_stream(&mut server_read, &mut client_write)
  )?;

  Ok(())
}

async fn get_remote_addr(tcp_stream : &TcpStream, round_robin_counter: Arc<Mutex<u32>>) -> String {
  let remote_ip = match LB_MEHOD {
    "Static Random" => {
      let mut hasher = DefaultHasher::new();
      tcp_stream.peer_addr().unwrap().ip().hash(&mut hasher);
      let ind = (hasher.finish() % REMOTE_ADDRESSES.len() as u64) as usize;
      REMOTE_ADDRESSES[ind]
    }
    "Round Robin" => {
      let mut rrc = round_robin_counter.lock().unwrap();
      *rrc = (*rrc+1) % REMOTE_ADDRESSES.len() as u32;
      REMOTE_ADDRESSES[*rrc as usize]
    }
    "Http Host" => { unimplemented!() }
    _ => "" // assuming we do config validitation somewhere else, this case will never happen
  };
  remote_ip.to_string()
}

async fn pipe_stream<R, W>(mut reader: R, mut writer: W) -> Result<(), io::Error>
where
  R: AsyncReadExt + Unpin,
  W: AsyncWriteExt + Unpin,
{
  let mut buffer = BytesMut::with_capacity(4 << 10); // 4096

  loop {
    match reader.read_buf(&mut buffer).await? {
      n if n == 0 => {
        break writer.shutdown().await;
      }
      _ => {
        trace!("PIPE: {}", std::string::String::from_utf8_lossy(&buffer[..]));
        writer.write_buf(&mut buffer).await?;
      }
    }
  }
}
