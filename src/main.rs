use admin::service::AdminService;
use pingora::prelude::*;
use proxy::GatewayProxy;
use service::{GlobalBackgroundService, ProxyService};
use tracing::{info, Level};

mod admin;
mod r#const;
mod docker;
mod lb;
mod proxy;
mod rate_limit;
mod service;
mod store;

fn main() {
    let trace = tracing_subscriber::fmt()
        .compact()
        .with_max_level(Level::INFO)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_ansi(false);

    let _ = trace.try_init();

    let mut my_server = Server::new(None).unwrap();
    my_server.bootstrap();
    my_server.add_service(GlobalBackgroundService::new());
    my_server.add_service(ProxyService::new());
    my_server.add_service(AdminService::new());

    let mut proxy_service = http_proxy_service(&my_server.configuration, GatewayProxy::new());
    proxy_service.add_tcp("0.0.0.0:6188");
    my_server.add_service(proxy_service);
    info!("Starting Pingora server with port: 6188");
    my_server.run_forever();
}
