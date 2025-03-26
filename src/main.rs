use pingora::prelude::*;
use proxy::GatewayProxy;
use service::{AdminService, GlobalBackgroundService, ProxyService};

mod admin;
mod lb;
mod proxy;
mod service;
mod service_discovery;
mod store;

fn main() {
    let mut my_server = Server::new(None).unwrap();
    my_server.bootstrap();
    my_server.add_service(GlobalBackgroundService::new());
    my_server.add_service(ProxyService::new());
    my_server.add_service(AdminService::new());

    let mut proxy_service = http_proxy_service(&my_server.configuration, GatewayProxy::new());
    proxy_service.add_tcp("0.0.0.0:6188");
    my_server.add_service(proxy_service);
    my_server.run_forever();
}
