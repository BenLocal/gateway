use std::sync::Arc;

use async_trait::async_trait;
use pingora::{
    server::{ListenFds, ShutdownWatch},
    services::Service,
};
use tracing::{error, info};

use crate::{
    docker::{background::DockerBackgroundService, servicediscovery::DockerServiceDiscovery},
    lb::{GatewayLoadBalancerOptions, GatewayMatchRule},
    store::ProxyCmd,
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
                    GatewayMatchRule::PathStartsWith("/healthz".to_string()),
                    pingora::lb::discovery::Static::try_from_iter(&vec!["127.0.0.1:3000"]).unwrap(),
                    false,
                );

                if let Err(e) =
                    crate::store::proxy_cmd(ProxyCmd::Add("admin".to_string(), options)).await
                {
                    error!("err: {:?}", e);
                }

                // test docker
                test_docker().await;
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

async fn test_docker() {
    let docker_client = Arc::new(bollard::Docker::connect_with_defaults().unwrap());

    let options = GatewayLoadBalancerOptions::new(
        GatewayMatchRule::PathStartsWith("/docker".to_string()),
        Box::new(DockerServiceDiscovery::new("app", docker_client.clone())),
        true,
    );

    if let Err(e) = crate::store::proxy_cmd(ProxyCmd::Add("docker".to_string(), options)).await {
        error!("err: {:?}", e);
    }

    let service = DockerBackgroundService::new(docker_client.clone());
    let _ = crate::store::globalbackground_cmd(crate::store::GlobalBackgroundCmd::Add(
        "docker_background_service".to_string(),
        Box::new(Arc::new(service)),
    ))
    .await;
}
