use super::{Context, Middleware};
use crate::server::handle_internal_server_error;
use hyper::{
  header::LOCATION,
  http::uri::{Scheme, Uri},
  Body, Request, Response, StatusCode,
};

#[derive(Debug)]
pub struct HttpsRedirector {}

impl Middleware for HttpsRedirector {
  fn modify_request(&self, request: Request<Body>, _context: &Context) -> Result<Request<Body>, Response<Body>> {
    if request.uri().scheme() == Some(&Scheme::HTTP) {
      let (parts, _body) = request.into_parts();
      let mut uri_parts = parts.uri.into_parts();
      uri_parts.scheme = Some(Scheme::HTTPS);
      let https_uri = Uri::from_parts(uri_parts).map_err(handle_internal_server_error)?;

      let response = Response::builder()
        .status(StatusCode::MOVED_PERMANENTLY)
        .header(LOCATION, https_uri.to_string())
        .body(Body::empty())
        .map_err(handle_internal_server_error)?;
      Err(response)
    } else {
      Ok(request)
    }
  }
}
