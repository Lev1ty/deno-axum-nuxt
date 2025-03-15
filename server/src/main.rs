use axum::{Router, routing::get, serve};
use eyre::Result;
use futures_concurrency::prelude::*;
use httparse::{EMPTY_HEADER, Request};
use tap::prelude::*;
use tokio::{
  io::copy_bidirectional,
  net::{TcpListener, TcpStream},
  process::Command,
  spawn,
};
use tracing::{Instrument, error, info, info_span, level_filters::LevelFilter, trace};
use tracing_error::ErrorLayer;
use tracing_subscriber::{fmt, layer::SubscriberExt, registry, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
  registry()
    .with(ErrorLayer::default())
    .with(fmt::layer())
    .with(LevelFilter::TRACE)
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
    .pipe(|router| serve(listener, router));
  (
    (server, ui.wait().instrument(info_span!("ui"))).try_join(),
    proxy(TcpListener::bind("127.0.0.1:3002").await?),
  )
    .join()
    .await
    .pipe(|(result, _)| result)?;
  Ok(())
}

async fn proxy(listener: TcpListener) {
  loop {
    if let Ok((mut tcp_stream_src, socket_addr)) =
      listener.accept().await.inspect_err(|err| error!(?err))
    {
      trace!(%socket_addr);
      let mut msg = [0u8; 8192];
      if let Ok(msg_peek_len) = tcp_stream_src
        .peek(&mut msg)
        .await
        .inspect_err(|err| error!(?err))
      {
        trace!(msg_peek_len);
        let mut headers = [EMPTY_HEADER; 128];
        let mut request = Request::new(&mut headers);
        if let Ok(request_parse_len) = request.parse(&msg) {
          trace!(?request_parse_len);
          let addr_dst = request
            .path
            .inspect(|path| info!(%path))
            .filter(|path| path.starts_with("/api"))
            .map(|_| "127.0.0.1:3001")
            .unwrap_or("127.0.0.1:3000")
            .tap(|dst| info!(%dst));
          spawn(async move {
            if let Ok(mut tcp_stream_dst) = TcpStream::connect(addr_dst)
              .await
              .inspect_err(|err| error!(?err))
            {
              let _ = copy_bidirectional(&mut tcp_stream_src, &mut tcp_stream_dst)
                .await
                .inspect_err(|err| error!(?err));
            }
          });
        }
      }
    }
  }
}
