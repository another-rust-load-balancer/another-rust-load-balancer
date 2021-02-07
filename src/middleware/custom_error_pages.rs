use std::fs;
use async_trait::async_trait;
use hyper::{Body, header::CONTENT_LENGTH, Response};
use toml::value::Table;

use super::{Context, Middleware};

#[derive(Debug)]
pub struct CustomErrorPages {
  pub(crate) location : String,
  pub(crate) errors: Vec<i64>
}

#[async_trait]
impl Middleware for CustomErrorPages {
  fn modify_response(&self, response: Response<Body>,  _context: &Context) -> Response<Body> {
    if self.errors.contains(&(response.status().as_u16() as i64)) {
      self.replace_response(response)
    }
    else { response }
  }
}


impl CustomErrorPages {

  pub(crate) fn new(t: Table) -> Result<Box<dyn Middleware>, ()> {
    let location = t
      .get("location")
      .ok_or(())?
      .as_str()
      .ok_or(())?
      .to_string();
    let errors = t
      .get("errors")
      .ok_or(())?
      .as_array()
      .ok_or(())?
      .iter()
      .map(|x| x.as_integer())
      .filter(|x| x.is_some())
      .map(|x|x.unwrap())
      .collect::<Vec<_>>();

    Ok(Box::new(CustomErrorPages{location, errors}))
  }

  fn replace_response(&self, response : Response<Body>) -> Response<Body> {
    let custom_body = fs::read_to_string(self.location.clone()+ response.status().as_str() +".html");
    let canocial_body = format!("{} - {}\n", response.status().as_str(), response.status().canonical_reason().unwrap_or(""));
    let (mut parts, _) = response.into_parts();
    parts.headers.remove(CONTENT_LENGTH);
    match custom_body {
      Ok(payload) => {
        Response::from_parts(parts,  Body::from(payload))
      },
      Err(_) => {
        Response::from_parts(parts,  Body::from(canocial_body))
      }
    }
  }
}


#[cfg(test)]
mod tests {
  use super::*;
}
