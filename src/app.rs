use std::{sync::Arc, time::Duration};

use pingora_limits::rate::Rate;
use tracing::{error, info};

use crate::{
    config::{GatewayApplicationConfig, GatewayLoadBalancerConfig},
    docker::{background::DockerBackgroundService, servicediscovery::DockerServiceDiscovery},
    lb::{GatewayLoadBalancerOptions, GatewayMatchRule, PingoraServiceDiscovery},
    proxy::ProxyCmd,
    r#const::DOCKER_BACKGROUND_SERVICE_NAME,
    rate_limit::RateLimiter,
    service::GlobalBackgroundCmd,
    store::{self, docker_client, GatewayApplication},
};

pub struct Application {
    pub app_id: String,
    pub limit_interval_seconds: u64,
    pub limit: u32,
}

impl From<&GatewayApplicationConfig> for Application {
    fn from(config: &GatewayApplicationConfig) -> Self {
        Application {
            app_id: config.app_id.to_string(),
            limit_interval_seconds: config.limit_interval_seconds,
            limit: config.limit,
        }
    }
}

impl Into<GatewayApplication> for Application {
    fn into(self) -> GatewayApplication {
        GatewayApplication::new(RateLimiter::new(
            Rate::new(Duration::from_secs(self.limit_interval_seconds)),
            self.limit,
        ))
    }
}

pub async fn add_application(app: Application) {
    let app_name = app.app_id.clone();
    info!(
        "Application: {}, Max Requests per Second: {}, Limit: {}",
        app_name, app.limit_interval_seconds, app.limit
    );
    store::applications()
        .write()
        .await
        .insert(app_name, Arc::new(app.into()));
}

pub struct LbInfo {
    pub name: String,
    pub match_rule: LbMatchRuleInfo,
    pub rewrite: Option<LbRewriteInfo>,
    pub service_discovery: String,
    pub upstream: Option<Vec<String>>,
}

pub struct LbMatchRuleInfo {
    pub typ: String,
    pub value: String,
}

pub struct LbRewriteInfo {
    pub regex: String,
    pub replacement: String,
}

impl From<&GatewayLoadBalancerConfig> for LbInfo {
    fn from(config: &GatewayLoadBalancerConfig) -> Self {
        LbInfo {
            name: config.name.clone(),
            match_rule: LbMatchRuleInfo {
                typ: config.match_rule.typ.clone(),
                value: config.match_rule.value.clone(),
            },
            rewrite: config.rewrite.as_ref().map(|r| LbRewriteInfo {
                regex: r.regex.clone(),
                replacement: r.replacement.clone(),
            }),
            service_discovery: config.service_discovery.clone(),
            upstream: config.upstream.clone(),
        }
    }
}

pub async fn add_load_balancer(lb: LbInfo) {
    let match_rule = match lb.match_rule.typ.as_str() {
        "path_start_with" => GatewayMatchRule::PathStartsWith(lb.match_rule.value.to_string()),
        "path_regex" => {
            let regex = regex::Regex::new(&lb.match_rule.value).unwrap();
            GatewayMatchRule::PathStartsWith(regex.to_string())
        }
        _ => return,
    };

    let rewrite = if let Some(rewrite) = &lb.rewrite {
        let regex = regex::Regex::new(&rewrite.regex).unwrap();
        Some((regex, rewrite.replacement.clone()))
    } else {
        None
    };

    let (service_discovery, default_health_check): (PingoraServiceDiscovery, bool) = match lb
        .service_discovery
        .as_str()
    {
        "static" => (
            pingora::lb::discovery::Static::try_from_iter(lb.upstream.clone().unwrap_or_default())
                .unwrap(),
            false,
        ),
        "docker" => (
            Box::new(DockerServiceDiscovery::new(&lb.name, docker_client())),
            true,
        ),
        _ => return,
    };

    let mut options =
        GatewayLoadBalancerOptions::new(match_rule, service_discovery, default_health_check);

    if let Some((regex, replacement)) = rewrite {
        options = options.with_rewrite(regex, replacement);
    }

    if let Err(e) = crate::store::proxy_cmd(ProxyCmd::Add(lb.name.to_string(), options)).await {
        error!("err: {:?}", e);
    }
}

pub struct BackgroundServer {
    pub name: String,
}

impl From<String> for BackgroundServer {
    fn from(name: String) -> Self {
        BackgroundServer { name: name }
    }
}

pub async fn start_background_service(server: BackgroundServer) {
    // start docker background service
    let (key, service) = match server.name.as_str() {
        "docker" => (
            DOCKER_BACKGROUND_SERVICE_NAME,
            DockerBackgroundService::new(docker_client()),
        ),
        _ => {
            error!("Unknown background service: {}", server.name);
            return;
        }
    };

    // Register the background service with the global background service
    info!("Starting background service: {}", server.name);
    let _ = crate::store::globalbackground_cmd(GlobalBackgroundCmd::Add(
        key.to_string(),
        Box::new(Arc::new(service)),
    ))
    .await;
}
