use std::{sync::Arc, time::Duration};

use admin::service::AdminService;
use docker::{background::DockerBackgroundService, servicediscovery::DockerServiceDiscovery};
use lb::{GatewayLoadBalancerOptions, GatewayMatchRule, PingoraServiceDiscovery};
use pingora::prelude::*;
use pingora_limits::rate::Rate;
use proxy::{GatewayProxy, ProxyCmd};
use r#const::DOCKER_BACKGROUND_SERVICE_NAME;
use rate_limit::RateLimiter;
use service::{GlobalBackgroundCmd, GlobalBackgroundService, ProxyService};
use store::{docker_client, GatewayApplication};
use tracing::{error, info, Level};

mod admin;
mod config;
mod r#const;
mod docker;
mod lb;
mod proxy;
mod rate_limit;
mod service;
mod store;

fn main() {
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "gateway.yaml".to_string());

    let config = config::GatewayConfig::from_file(&config_path).unwrap_or_else(|e| {
        panic!("Failed to load config file: {:?}", e);
    });

    // Initialize the global configuration
    store::init_config(config);

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

    // init with tokio runtime
    run_with_tokio_runtime();

    my_server.run_forever();
}

fn run_with_tokio_runtime() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        init_config_to_proxy(&store::config()).await;
    });
}

async fn init_config_to_proxy(config: &config::GatewayConfig) {
    if let Some(applications) = &config.applications {
        for app in applications {
            let app_name = app.app_id.clone();
            info!(
                "Application: {}, Max Requests per Second: {}, Limit: {}",
                app_name, app.limit_interval_seconds, app.limit
            );
            store::applications().write().await.insert(
                app_name.clone(),
                Arc::new(GatewayApplication::new(RateLimiter::new(
                    Rate::new(Duration::from_secs(app.limit_interval_seconds)),
                    app.limit,
                ))),
            );
        }
    }

    if let Some(load_balancers) = &config.load_balancers {
        for lb in load_balancers {
            let match_rule = match lb.match_rule.typ.as_str() {
                "path_start_with" => {
                    GatewayMatchRule::PathStartsWith(lb.match_rule.value.to_string())
                }
                "path_regex" => {
                    let regex = regex::Regex::new(&lb.match_rule.value).unwrap();
                    GatewayMatchRule::PathStartsWith(regex.to_string())
                }
                _ => continue,
            };

            let rewrite = if let Some(rewrite) = &lb.rewrite {
                let regex = regex::Regex::new(&rewrite.regex).unwrap();
                Some((regex, rewrite.replacement.clone()))
            } else {
                None
            };

            let (service_discovery, default_health_check): (PingoraServiceDiscovery, bool) =
                match lb.service_discovery.as_str() {
                    "static" => (
                        pingora::lb::discovery::Static::try_from_iter(
                            lb.upstream.clone().unwrap_or_default(),
                        )
                        .unwrap(),
                        false,
                    ),
                    "docker" => {
                        // start docker background service
                        let service = DockerBackgroundService::new(docker_client());
                        let _ = crate::store::globalbackground_cmd(GlobalBackgroundCmd::Add(
                            DOCKER_BACKGROUND_SERVICE_NAME.to_string(),
                            Box::new(Arc::new(service)),
                        ))
                        .await;

                        (
                            Box::new(DockerServiceDiscovery::new(&lb.name, docker_client())),
                            true,
                        )
                    }
                    _ => continue,
                };

            let mut options = GatewayLoadBalancerOptions::new(
                match_rule,
                service_discovery,
                default_health_check,
            );

            if let Some((regex, replacement)) = rewrite {
                options = options.with_rewrite(regex, replacement);
            }

            if let Err(e) =
                crate::store::proxy_cmd(ProxyCmd::Add(lb.name.to_string(), options)).await
            {
                error!("err: {:?}", e);
            }
        }
    }
}
