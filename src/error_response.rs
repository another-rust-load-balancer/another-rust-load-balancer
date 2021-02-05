use hyper::{Body, Response, StatusCode};
use log::error;
use std::error::Error;

pub fn not_found() -> Response<Body> {
  Response::builder()
    .status(StatusCode::NOT_FOUND)
    .body(Body::from("404 - page not found"))
    .unwrap()
}

pub fn handle_bad_gateway<E: Error>(error: E) -> Response<Body> {
  log_error(error);
  bad_gateway()
}

pub fn bad_gateway() -> Response<Body> {
  Response::builder()
    .status(StatusCode::BAD_GATEWAY)
    .body(Body::empty())
    .unwrap()
}

pub fn bad_request<B>(message: B) -> Response<Body>
where
  Body: From<B>,
{
  Response::builder()
    .status(StatusCode::BAD_REQUEST)
    .body(Body::from(message))
    .unwrap()
}

pub fn handle_internal_server_error<E: Error>(error: E) -> Response<Body> {
  log_error(error);
  internal_server_error()
}

pub fn internal_server_error() -> Response<Body> {
  Response::builder()
    .status(StatusCode::INTERNAL_SERVER_ERROR)
    .body(Body::empty())
    .unwrap()
}

pub fn log_error<E: Error>(error: E) {
  error!("{}", error);
}

pub fn request_entity_to_large() -> Response<Body> {
  Response::builder()
      .status(StatusCode::PAYLOAD_TOO_LARGE)
      .body(Body::empty())
      .unwrap()
}
