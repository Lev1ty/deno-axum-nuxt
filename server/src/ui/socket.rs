use axum::{
  extract::ws::{self, WebSocket},
  http::request::Parts,
};
use derive_more::Display;
use futures::StreamExt;
use futures_concurrency::future::Join;
use std::future::ready;
use tap::prelude::*;
use thiserror::Error;
use tokio_tungstenite::{
  connect_async,
  tungstenite::{self, client::IntoClientRequest},
};
use tracing::{Level, error, instrument};

pub async fn handle(parts: Parts, mut socket: WebSocket) {
  let _ = handle_fallible(parts, &mut socket).await;
}

#[instrument(err(level = Level::DEBUG))]
async fn handle_fallible(parts: Parts, socket: &mut WebSocket) -> Result<(), Error> {
  let (stream, _) = ("ws://localhost:3000", parts.uri.path(), parts.uri.query())
    .pipe(|(uri, path, query)| format!("{uri}{path}?{}", query.unwrap_or_default()))
    .into_client_request()?
    .tap_mut(|builder| *builder.headers_mut() = parts.headers)
    .pipe(connect_async)
    .await?;
  let (socket_rx, socket_tx) = socket.split();
  let (stream_rx, stream_tx) = stream.split();
  let upstream = socket_tx
    .filter_map(|result| ready(result.inspect_err(|err| error!(?err)).ok()))
    .map(|msg| {
      Ok(match msg {
        ws::Message::Text(str) => tungstenite::Message::Text(str.to_string().into()),
        ws::Message::Binary(bytes) => tungstenite::Message::Binary(bytes),
        ws::Message::Ping(bytes) => tungstenite::Message::Ping(bytes),
        ws::Message::Pong(bytes) => tungstenite::Message::Pong(bytes),
        ws::Message::Close(c) => {
          tungstenite::Message::Close(c.map(|c| tungstenite::protocol::CloseFrame {
            code: c.code.into(),
            reason: tungstenite::Utf8Bytes::from(c.reason.to_string()),
          }))
        }
      })
    })
    .forward(stream_rx);
  let downstream = stream_tx
    .filter_map(|result| ready(result.inspect_err(|err| error!(?err)).ok()))
    .map(|msg| {
      Ok(match msg {
        tungstenite::Message::Text(str) => ws::Message::Text(str.to_string().into()),
        tungstenite::Message::Binary(bytes) => ws::Message::Binary(bytes),
        tungstenite::Message::Ping(bytes) => ws::Message::Ping(bytes),
        tungstenite::Message::Pong(bytes) => ws::Message::Pong(bytes),
        tungstenite::Message::Close(c) => ws::Message::Close(c.map(|c| ws::CloseFrame {
          code: c.code.into(),
          reason: c.reason.to_string().into(),
        })),
        tungstenite::Message::Frame(_) => unreachable!(),
      })
    })
    .forward(socket_rx);
  let (upstream, downstream) = (upstream, downstream).join().await;
  upstream?;
  downstream?;
  Ok(())
}

#[derive(Debug, Display, Error)]
enum Error {
  Axum(#[from] axum::Error),
  Tungstenite(#[from] tokio_tungstenite::tungstenite::Error),
}
