use async_stream::stream;
use async_trait::async_trait;
use futures::Stream;
use log::error;
use std::{
  io,
  net::SocketAddr,
  pin::Pin,
  sync::Arc,
  task::{Context, Poll},
};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::server::TlsStream;
use tokio_rustls::{rustls::ServerConfig, TlsAcceptor};

pub struct HyperAcceptor<'a, T> {
  acceptor: Pin<Box<dyn Stream<Item = Result<T, io::Error>> + Send + 'a>>,
}

impl hyper::server::accept::Accept for HyperAcceptor<'_, TcpStream> {
  type Conn = TcpStream;
  type Error = io::Error;

  fn poll_accept(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
    Pin::new(&mut self.acceptor).poll_next(cx)
  }
}

impl hyper::server::accept::Accept for HyperAcceptor<'_, TlsStream<TcpStream>> {
  type Conn = TlsStream<TcpStream>;
  type Error = io::Error;

  fn poll_accept(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
    Pin::new(&mut self.acceptor).poll_next(cx)
  }
}

#[async_trait]
pub trait AcceptorProducer<T> {
  async fn produce_acceptor(self, address: &str) -> Result<HyperAcceptor<'async_trait, T>, io::Error>;
}

pub struct Http;

#[async_trait]
impl AcceptorProducer<TcpStream> for Http {
  async fn produce_acceptor(self, address: &str) -> Result<HyperAcceptor<'async_trait, TcpStream>, io::Error> {
    let listener = TcpListener::bind(address).await?;

    let incoming_stream = stream! {
      loop {
          let (socket, _) = listener.accept().await?;
          yield Ok(socket);
      }
    };

    Ok(HyperAcceptor {
      acceptor: Box::pin(incoming_stream),
    })
  }
}

pub struct Https {
  pub tls_config: ServerConfig,
}

#[async_trait]
impl AcceptorProducer<TlsStream<TcpStream>> for Https {
  async fn produce_acceptor(
    self,
    address: &str,
  ) -> Result<HyperAcceptor<'async_trait, TlsStream<TcpStream>>, io::Error> {
    let tls_acceptor = TlsAcceptor::from(Arc::new(self.tls_config));
    let listener = TcpListener::bind(address).await?;

    let incoming_stream = stream! {
      loop {
          let (socket, _) = listener.accept().await?;
          match tls_acceptor.accept(socket).await {
            Ok(tls_stream) => yield Ok(tls_stream),
            Err(e) => error!("Failed to accept TLS socket: {}", e)
          }
      }
    };

    Ok(HyperAcceptor {
      acceptor: Box::pin(incoming_stream),
    })
  }
}

pub trait RemoteAddress {
  fn remote_addr(&self) -> io::Result<SocketAddr>;
}

impl RemoteAddress for TcpStream {
  fn remote_addr(&self) -> io::Result<SocketAddr> {
    self.peer_addr()
  }
}

impl RemoteAddress for TlsStream<TcpStream> {
  fn remote_addr(&self) -> io::Result<SocketAddr> {
    let (stream, _) = self.get_ref();
    stream.peer_addr()
  }
}
