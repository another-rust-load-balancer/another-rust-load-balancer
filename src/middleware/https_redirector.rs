use super::{Context, Middleware};
use crate::{
  error_response::{bad_request, handle_internal_server_error, internal_server_error},
  server,
};
use hyper::{
  header::{HOST, LOCATION},
  http::uri::{Authority, Scheme, Uri},
  Body, Request, Response, StatusCode,
};
use log::debug;
use std::convert::TryFrom;

#[derive(Debug)]
pub struct HttpsRedirector;

impl Middleware for HttpsRedirector {
  fn modify_request(&self, request: Request<Body>, context: &Context) -> Result<Request<Body>, Response<Body>> {
    match context.client_scheme {
      server::Scheme::HTTP => {
        let host_authority = parse_host_header(&request)?;

        let (parts, _body) = request.into_parts();
        let uri_parts = parts.uri.into_parts();
        let authority = uri_parts.authority.unwrap_or(host_authority);
        let path_and_query = uri_parts.path_and_query.ok_or_else(internal_server_error)?;
        let https_uri = Uri::builder()
          .scheme(Scheme::HTTPS)
          .authority(authority)
          .path_and_query(path_and_query)
          .build()
          .map_err(handle_internal_server_error)?;

        let response = Response::builder()
          .status(StatusCode::MOVED_PERMANENTLY)
          .header(LOCATION, https_uri.to_string())
          .body(Body::empty())
          .map_err(handle_internal_server_error)?;

        debug!("Redirecting to {}", https_uri);

        Err(response)
      }
      _ => Ok(request),
    }
  }
}

fn parse_host_header(request: &Request<Body>) -> Result<Authority, Response<Body>> {
  let host = request
    .headers()
    .get(HOST)
    .ok_or_else(|| bad_request("missing host header"))?
    .to_str()
    .map_err(|error| bad_request(error.to_string()))?;
  Authority::try_from(host).map_err(|_error| bad_request("invalid host header"))
}
