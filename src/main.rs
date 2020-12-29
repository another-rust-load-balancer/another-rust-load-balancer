use bytes::BytesMut;
use log::{trace, LevelFilter};
use log4rs::{
  append::console::ConsoleAppender,
  config::{Appender, Root},
  Config,
};
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::{
    tcp::{ReadHalf, WriteHalf},
    TcpListener, TcpStream,
  },
};

static LOCAL_ADDRESS: &str = "127.0.0.1:3000";
static REMOTE_ADDRESS: &str = "127.0.0.1:8081";

#[tokio::main]
pub async fn main() -> Result<(), std::io::Error> {
  let stdout = ConsoleAppender::builder().build();

  let config = Config::builder()
    .appender(Appender::builder().build("stdout", Box::new(stdout)))
    .build(Root::builder().appender("stdout").build(LevelFilter::Trace))
    .unwrap();

  log4rs::init_config(config).expect("Logging should not fail");

  let listener = TcpListener::bind(LOCAL_ADDRESS).await?;

  loop {
    let (stream, _) = listener.accept().await?;

    tokio::spawn(process_stream(stream));
  }
}

async fn process_stream(mut stream: TcpStream) -> Result<(), std::io::Error> {
  let (client_read, client_write) = stream.split();

  let mut remote = TcpStream::connect(REMOTE_ADDRESS).await?;
  let (remote_read, remote_write) = remote.split();

  tokio::try_join!(
    pipe_stream(client_read, remote_write),
    pipe_stream(remote_read, client_write)
  )?;

  Ok(())
}

async fn pipe_stream(mut reader: ReadHalf<'_>, mut writer: WriteHalf<'_>) -> Result<(), std::io::Error> {
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
