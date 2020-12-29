use bytes::BytesMut;
use log::{trace, LevelFilter};
use log4rs::{
  append::console::ConsoleAppender,
  config::{Appender, Root},
  Config,
};
use std::{fs::File, io::BufReader, path::Path, sync::Arc};
use tokio::{
  io::{split, AsyncReadExt, AsyncWriteExt},
  net::{TcpListener, TcpStream},
};
use tokio_rustls::{
  rustls::{
    internal::pemfile::{certs, rsa_private_keys},
    Certificate, NoClientAuth, PrivateKey, ServerConfig,
  },
  TlsAcceptor,
};

static LOCAL_ADDRESS: &str = "127.0.0.1:3000";
static REMOTE_ADDRESS: &str = "www.google.de:80";

#[tokio::main]
pub async fn main() -> Result<(), std::io::Error> {
  let stdout = ConsoleAppender::builder().build();

  let config = Config::builder()
    .appender(Appender::builder().build("stdout", Box::new(stdout)))
    .build(Root::builder().appender("stdout").build(LevelFilter::Trace))
    .unwrap();

  log4rs::init_config(config).expect("Logging should not fail");

  let certs = load_certs(Path::new("x509/server.cer"))?;
  let mut keys = load_keys(Path::new("x509/server.key"))?;
  let mut tls_config = ServerConfig::new(NoClientAuth::new());
  tls_config
    .set_single_cert(certs, keys.remove(0))
    .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
  let tls_acceptor = TlsAcceptor::from(Arc::new(tls_config));

  let listener = TcpListener::bind(LOCAL_ADDRESS).await?;

  loop {
    let (stream, _) = listener.accept().await?;
    let tls_acceptor = tls_acceptor.clone();
    tokio::spawn(process_stream(stream, tls_acceptor));
  }
}

fn load_certs(path: &Path) -> std::io::Result<Vec<Certificate>> {
  certs(&mut BufReader::new(File::open(path)?))
    .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid cert"))
}

fn load_keys(path: &Path) -> std::io::Result<Vec<PrivateKey>> {
  rsa_private_keys(&mut BufReader::new(File::open(path)?))
    .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid key"))
}

async fn process_stream(stream: TcpStream, tls_acceptor: TlsAcceptor) -> Result<(), std::io::Error> {
  let tls_stream = tls_acceptor.accept(stream).await?;
  let (mut client_read, mut client_write) = split(tls_stream);

  let mut remote = TcpStream::connect(REMOTE_ADDRESS).await?;
  let (mut remote_read, mut remote_write) = remote.split();

  tokio::try_join!(
    pipe_stream(&mut client_read, &mut remote_write),
    pipe_stream(&mut remote_read, &mut client_write)
  )?;

  Ok(())
}

async fn pipe_stream<R, W>(mut reader: R, mut writer: W) -> Result<(), std::io::Error>
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
