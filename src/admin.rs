use axum::{response::Html, routing::get, Router};
use tokio_util::sync::CancellationToken;

pub async fn start_admin_server(cancel: CancellationToken) -> anyhow::Result<()> {
    let app = Router::new().route("/healthz", get(handler));

    // run it
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("admin server listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = cancel.cancelled() => {
                    println!("admin server shutdown");
                },
            }
        })
        .await?;

    Ok(())
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}
