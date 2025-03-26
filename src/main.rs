use pingora::prelude::*;
use proxy::GatewayProxy;
use service::{AdminService, GlobalBackgroundService, ProxyService};
use tokio_util::sync::CancellationToken;

mod admin;
mod lb;
mod proxy;
mod service;
mod service_discovery;
mod store;

fn main() {
    let mut my_server = Server::new(None).unwrap();
    let cancel = CancellationToken::new();
    my_server.bootstrap();
    my_server.add_service(GlobalBackgroundService::new());
    my_server.add_service(ProxyService::new());
    my_server.add_service(AdminService::new(cancel.clone()));

    let mut proxy_service = http_proxy_service(&my_server.configuration, GatewayProxy::new());
    proxy_service.add_tcp("0.0.0.0:6188");
    my_server.add_service(proxy_service);
    my_server.run_forever();
}
