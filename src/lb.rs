use std::sync::Arc;

use async_trait::async_trait;
use pingora::{
    lb::{discovery::ServiceDiscovery, Backends, LoadBalancer},
    prelude::*,
    server::ShutdownWatch,
    services::background::BackgroundService,
};
use regex::Regex;

pub type PingoraServiceDiscovery = Box<dyn ServiceDiscovery + Send + Sync + 'static>;

pub struct GatewayLoadBalancerOptions {
    pub match_rule: GatewayMatchRule,
    pub service_discovery: PingoraServiceDiscovery,
    pub health_check: bool,
    pub rewrite: Option<(Regex, String)>,
}

impl GatewayLoadBalancerOptions {
    pub fn new(
        match_rule: GatewayMatchRule,
        service_discovery: PingoraServiceDiscovery,
        health_check: bool,
    ) -> Self {
        Self {
            match_rule,
            service_discovery,
            health_check,
            rewrite: None,
        }
    }

    pub fn with_rewrite(mut self, regex: Regex, replacement: String) -> Self {
        self.rewrite = Some((regex, replacement));
        self
    }
}

pub struct GatewayLoadBalancer {
    name: String,
    match_rule: GatewayMatchRule,
    rewrite: Option<(Regex, String)>,
    inner: Arc<LoadBalancer<RoundRobin>>,
}

impl GatewayLoadBalancer {
    pub fn new(name: &str, options: GatewayLoadBalancerOptions) -> Self {
        let backends = Backends::new(options.service_discovery);
        let mut upstreams = LoadBalancer::from_backends(backends);

        if options.health_check {
            upstreams.set_health_check(TcpHealthCheck::new());
            upstreams.health_check_frequency = Some(std::time::Duration::from_secs(1));
        }

        upstreams.update_frequency = Some(std::time::Duration::from_secs(5));
        Self {
            name: name.to_string(),
            match_rule: options.match_rule,
            inner: Arc::new(upstreams),
            rewrite: options.rewrite,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn matches_path(&self, path: &str) -> bool {
        self.match_rule.matches_path(path)
    }

    pub fn lb(&self) -> Arc<LoadBalancer<RoundRobin>> {
        self.inner.clone()
    }

    pub fn rewrite_path(&self, path: &str) -> Option<String> {
        if let Some((regex, replacement)) = &self.rewrite {
            if regex.is_match(path) {
                return Some(regex.replace(path, replacement).to_string());
            }
        }
        None
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
    #[allow(dead_code)]
    PathRegex(Regex),
}

impl GatewayMatchRule {
    pub fn matches_path(&self, path: &str) -> bool {
        match self {
            GatewayMatchRule::PathStartsWith(prefix) => path.starts_with(prefix),
            GatewayMatchRule::PathRegex(regex) => regex.is_match(path),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_gateway_match_rule() {
        let rule_starts_with = GatewayMatchRule::PathStartsWith("/api".to_string());
        let rule_regex = GatewayMatchRule::PathRegex(Regex::new(r"^/user/\d+$").unwrap());

        let path1 = "/api/v1/resource";
        let path2 = "/user/123";

        assert!(rule_starts_with.matches_path(path1));
        assert!(rule_regex.matches_path(path2));
    }
}
