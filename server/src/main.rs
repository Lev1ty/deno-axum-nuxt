mod ui;

use axum::{
  Router,
  routing::{any, get},
  serve,
};
use eyre::Result;
use futures_concurrency::prelude::*;
use tap::prelude::*;
use tokio::{net::TcpListener, process::Command};
use tracing::{Instrument, error, info_span, level_filters::LevelFilter};
use tracing_error::ErrorLayer;
use tracing_subscriber::{fmt, layer::SubscriberExt, registry, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
  registry()
    .with(ErrorLayer::default())
    .with(fmt::layer())
    .with(LevelFilter::INFO)
    .try_init()?;
  let mut ui = Command::new("deno")
    .arg("task")
    .arg("dev")
    .current_dir(format!("{}/../ui", env!("CARGO_MANIFEST_DIR")))
    .env("EDITOR", "zed")
    .spawn()
    .inspect_err(|err| error!(?err))?;
  let listener = TcpListener::bind("127.0.0.1:3001").await?;
  let server = Router::new()
    .route("/api/hello", get(|| async { "Hello, world!" }))
    .fallback(any(ui::handle))
    .pipe(|router| serve(listener, router));
  (server, ui.wait().instrument(info_span!("ui")))
    .try_join()
    .await?;
  Ok(())
}
