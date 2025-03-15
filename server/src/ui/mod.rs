mod socket;

use axum::{
  body::Body,
  extract::{self, FromRequestParts, WebSocketUpgrade},
  http::{
    self, Uri,
    uri::{Authority, InvalidUriParts, Scheme},
  },
  response::{IntoResponse, Response},
};
use derive_more::Display;
use reqwest::{Client, Request, StatusCode, Url};
use std::mem;
use tap::prelude::*;
use thiserror::Error;
use tracing::{Level, error, instrument};

#[instrument]
pub async fn handle(request: extract::Request) -> Result<Response, Error> {
  let (mut parts, body) = request.into_parts();
  #[cfg(debug_assertions)]
  if let Ok(upgrade) = WebSocketUpgrade::from_request_parts(&mut parts, &mut ()).await {
    return upgrade
      .on_failed_upgrade(|err| error!(?err))
      .on_upgrade(|socket| socket::handle(parts, socket))
      .pipe(Ok);
  }
  let mut uri_parts = parts.uri.into_parts();
  uri_parts
    .authority
    .replace(Authority::from_static("localhost:3000"));
  uri_parts.scheme.replace(Scheme::HTTP);
  let mut response = Uri::from_parts(uri_parts)?
    .to_string()
    .parse::<Url>()?
    .pipe(|uri| Request::new(parts.method, uri))
    .tap_mut(|builder| {
      *builder.headers_mut() = parts.headers;
    })
    .tap_mut(|builder| {
      builder
        .body_mut()
        .replace(reqwest::Body::wrap_stream(body.into_data_stream()));
    })
    .pipe(|request| Client::new().execute(request))
    .await?;
  Response::builder()
    .status(response.status())
    .tap_mut(|builder| {
      builder.headers_mut().map(|builder| {
        *builder = response.headers_mut().pipe(mem::take);
      });
    })
    .body(Body::from_stream(response.bytes_stream()))?
    .pipe(Ok)
}

#[derive(Debug, Display, Error)]
pub enum Error {
  Http(#[from] http::Error),
  InvalidUriParts(#[from] InvalidUriParts),
  Reqwest(#[from] reqwest::Error),
  Tungstenite(#[from] tokio_tungstenite::tungstenite::Error),
  Url(#[from] url::ParseError),
}

impl IntoResponse for Error {
  #[instrument(ret, level = Level::ERROR)]
  fn into_response(self) -> Response {
    match self {
      Self::Http(_)
      | Self::InvalidUriParts(_)
      | Self::Reqwest(_)
      | Self::Tungstenite(_)
      | Self::Url(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
  }
}
