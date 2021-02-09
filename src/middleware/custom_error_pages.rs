use super::{Context, Middleware};
use async_trait::async_trait;
use hyper::{
  header::{CONTENT_ENCODING, CONTENT_LENGTH},
  Body, Response,
};
use log::error;
use std::{convert::TryFrom, fs, path::Path};
use toml::value::Table;

#[derive(Debug)]
pub struct CustomErrorPages {
  location: String,
  errors: Vec<u16>,
}

#[async_trait]
impl Middleware for CustomErrorPages {
  fn modify_response(&self, response: Response<Body>, _context: &Context) -> Response<Body> {
    if self.errors.contains(&(response.status().as_u16())) {
      self.replace_response(response)
    } else {
      response
    }
  }
}

impl TryFrom<Table> for CustomErrorPages {
  type Error = ();

  fn try_from(t: Table) -> Result<Self, Self::Error> {
    let location = t.get("location").ok_or(())?.as_str().ok_or(())?.to_string();
    let errors = t
      .get("errors")
      .ok_or(())?
      .as_array()
      .ok_or(())?
      .iter()
      .map(|x| x.as_integer())
      .filter(|x| x.is_some())
      .map(|x| x.unwrap())
      .map(|x| x as u16)
      .collect::<Vec<_>>();

    Ok(CustomErrorPages { location, errors })
  }
}

impl CustomErrorPages {
  fn replace_response(&self, response: Response<Body>) -> Response<Body> {
    let filepath = Path::new(&self.location)
      .join(response.status().as_str())
      .with_extension("html");
    let custom_body = fs::read_to_string(filepath);
    let canocial_body = format!(
      "{} - {}\n",
      response.status().as_str(),
      response.status().canonical_reason().unwrap_or("")
    );
    let (mut parts, _) = response.into_parts();
    parts.headers.remove(CONTENT_LENGTH);
    parts.headers.remove(CONTENT_ENCODING);
    match custom_body {
      Ok(payload) => Response::from_parts(parts, Body::from(payload)),
      Err(_) => {
        error!("Custom error page for error {} not found!", parts.status.as_str());
        Response::from_parts(parts, Body::from(canocial_body))
      }
    }
  }
}
