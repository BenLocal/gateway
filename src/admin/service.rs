use async_trait::async_trait;
use pingora::{
    server::{ListenFds, ShutdownWatch},
    services::Service,
};
use tracing::{error, info};

use crate::{
    lb::{GatewayLoadBalancerOptions, GatewayMatchRule},
    proxy::ProxyCmd,
};

use super::serve::start_admin_server;

pub struct AdminService;

impl AdminService {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Service for AdminService {
    async fn start_service(&mut self, _fds: Option<ListenFds>, shutdown: ShutdownWatch) {
        loop {
            tokio::spawn(async {
                let options = GatewayLoadBalancerOptions::new(
                    GatewayMatchRule::PathStartsWith("/admin".to_string()),
                    pingora::lb::discovery::Static::try_from_iter(&vec!["127.0.0.1:3000"]).unwrap(),
                    false,
                )
                .with_rewrite(regex::Regex::new("^/admin").unwrap(), "".to_string());

                if let Err(e) =
                    crate::store::proxy_cmd(ProxyCmd::Add("admin".to_string(), options)).await
                {
                    error!("err: {:?}", e);
                }
            });

            let _ = start_admin_server(shutdown.clone()).await;

            {
                info!("remove admin route");
                if let Err(e) = crate::store::proxy_cmd(ProxyCmd::Remove("admin".to_string())).await
                {
                    error!("err: {:?}", e);
                }
            }
        }
    }

    fn name(&self) -> &str {
        "AdminService"
    }

    fn threads(&self) -> Option<usize> {
        Some(1)
    }
}
