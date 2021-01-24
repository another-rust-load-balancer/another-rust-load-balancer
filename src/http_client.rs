use std::{
  io,
  pin::Pin,
  sync::Arc,
  task::{Context, Poll},
};

use futures::Future;
use hyper::{
  client::{connect::Connection, HttpConnector},
  http::uri::Uri,
  service::Service,
};
use pin_project::{pin_project, pinned_drop};
use tokio::{
  io::{AsyncRead, AsyncWrite},
  net::TcpStream,
};

use crate::load_balancing::LoadBalancingStrategy;

/// A wrapper around any async stream. Notifies the given strategy once the stream is closed
#[pin_project(PinnedDrop)]
pub struct StrategyNotifyStream<T: AsyncRead + AsyncWrite + Connection + Send> {
  #[pin]
  inner: T,
  target: Uri,
  strategy: Arc<Box<dyn LoadBalancingStrategy>>,
}

impl<T: AsyncRead + AsyncWrite + Connection + Send> StrategyNotifyStream<T> {
  pub fn new(inner: T, target: Uri, strategy: Arc<Box<dyn LoadBalancingStrategy>>) -> Self {
    StrategyNotifyStream {
      inner,
      target,
      strategy,
    }
  }
}

#[pinned_drop]
impl<T: AsyncRead + AsyncWrite + Connection + Send> PinnedDrop for StrategyNotifyStream<T> {
  fn drop(self: Pin<&mut Self>) {
    self.strategy.on_tcp_close(&self.target);
  }
}

impl<T: AsyncRead + AsyncWrite + Connection + Send + Sync> AsyncRead for StrategyNotifyStream<T> {
  fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut tokio::io::ReadBuf<'_>) -> Poll<io::Result<()>> {
    self.project().inner.poll_read(cx, buf)
  }
}

impl<T: AsyncRead + AsyncWrite + Connection + Send + Sync> AsyncWrite for StrategyNotifyStream<T> {
  fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, io::Error>> {
    self.project().inner.poll_write(cx, buf)
  }

  fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
    self.project().inner.poll_flush(cx)
  }

  fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
    self.project().inner.poll_shutdown(cx)
  }
}
impl<T: AsyncRead + AsyncWrite + Connection + Send + Sync> Connection for StrategyNotifyStream<T> {
  fn connected(&self) -> hyper::client::connect::Connected {
    self.inner.connected()
  }
}

#[derive(Clone, Debug)]
pub struct StrategyNotifyHttpConnector {
  inner: HttpConnector,
  strategy: Arc<Box<dyn LoadBalancingStrategy>>,
}

impl StrategyNotifyHttpConnector {
  pub fn new(strategy: Arc<Box<dyn LoadBalancingStrategy>>) -> StrategyNotifyHttpConnector {
    StrategyNotifyHttpConnector {
      inner: HttpConnector::new(),
      strategy,
    }
  }
}

impl Service<Uri> for StrategyNotifyHttpConnector {
  type Response = StrategyNotifyStream<TcpStream>;

  type Error = Box<dyn std::error::Error + Send + Sync>;

  // let's allow this complex type. A refactor would make it more complicated due to the used trait types
  #[allow(clippy::type_complexity)]
  type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

  fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    self.inner.poll_ready(cx).map_err(|e| e.into())
  }

  fn call(&mut self, req: Uri) -> Self::Future {
    let mut self_ = self.clone();
    let req_ = req.clone();

    Box::pin(async move {
      match self_.inner.call(req).await {
        Ok(stream) => {
          self_.strategy.on_tcp_open(&req_);
          Ok(StrategyNotifyStream::new(stream, req_.clone(), self_.strategy))
        }
        Err(e) => Err(e.into()),
      }
    })
  }
}
