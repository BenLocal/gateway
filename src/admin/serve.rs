use axum::{response::Html, routing::get, Router};
use pingora::server::ShutdownWatch;
use tracing::info;

pub async fn start_admin_server(mut shutdown: ShutdownWatch) -> anyhow::Result<()> {
    let app = Router::new().route("/healthz", get(handler));

    // run it
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    info!("admin server listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = shutdown.changed() => {
                  info!("admin server shutdown");
                },
            }
        })
        .await?;

    Ok(())
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}
