use lb::GatewayMatchRule;
use pingora::prelude::*;
use proxy::GatewayProxy;
use service::{GlobalBackgroundService, ProxyService};
use store::ProxyCmd;

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

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _ = rt.block_on(async {
        if let Err(e) = store::proxy_cmd(ProxyCmd::Add(
            "test".to_string(),
            GatewayMatchRule::PathStartsWith("/healthz".to_string()),
        ))
        .await
        {
            println!("err: {:?}", e);
        }

        tokio::spawn(async {
            tokio::time::sleep(tokio::time::Duration::from_secs(50)).await;
            println!("remove test");
            if let Err(e) = store::proxy_cmd(ProxyCmd::Remove("test".to_string())).await {
                println!("err: {:?}", e);
            }
        })
    });

    let mut proxy_service = http_proxy_service(&my_server.configuration, GatewayProxy::new());
    proxy_service.add_tcp("0.0.0.0:6188");
    my_server.add_service(proxy_service);
    my_server.run_forever();
}
