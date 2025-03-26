use std::sync::Arc;

use async_trait::async_trait;
use pingora::{
    lb::{Backends, LoadBalancer},
    prelude::*,
    server::ShutdownWatch,
    services::background::BackgroundService,
};

use crate::service_discovery::GatewayServiceDiscovery;

pub struct GatewayLoadBalancer {
    name: String,
    match_rule: GatewayMatchRule,
    inner: Arc<LoadBalancer<RoundRobin>>,
}

impl GatewayLoadBalancer {
    pub fn new(name: &str, match_rule: GatewayMatchRule) -> Self {
        let backends = Backends::new(Box::new(GatewayServiceDiscovery::new()));
        let mut upstreams = LoadBalancer::from_backends(backends);
        upstreams.set_health_check(TcpHealthCheck::new());
        upstreams.health_check_frequency = Some(std::time::Duration::from_secs(1));
        upstreams.update_frequency = Some(std::time::Duration::from_secs(5));
        Self {
            name: name.to_string(),
            match_rule,
            inner: Arc::new(upstreams),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn match_rule(&self) -> &GatewayMatchRule {
        &self.match_rule
    }

    pub fn lb(&self) -> Arc<LoadBalancer<RoundRobin>> {
        self.inner.clone()
    }
}

#[async_trait]
impl BackgroundService for GatewayLoadBalancer {
    async fn start(&self, shutdown: ShutdownWatch) {
        self.inner.start(shutdown).await;
    }
}

pub enum GatewayMatchRule {
    PathStartsWith(String),
}
