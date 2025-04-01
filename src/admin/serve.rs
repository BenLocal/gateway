use std::{sync::Arc, time::Duration};

use axum::{
    response::Html,
    routing::{get, post},
    Json, Router,
};
use pingora::server::ShutdownWatch;
use pingora_limits::rate::Rate;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{
    docker::{background::DockerBackgroundService, servicediscovery::DockerServiceDiscovery},
    lb::{GatewayLoadBalancerOptions, GatewayMatchRule, PingoraServiceDiscovery},
    r#const::DOCKER_BACKGROUND_SERVICE_NAME,
    rate_limit::RateLimiter,
    store::{self, docker_client, ProxyCmd},
};

pub async fn start_admin_server(mut shutdown: ShutdownWatch) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/healthz", get(handler))
        .route("/app/add", post(add_application))
        .route("/app/update", post(update_application))
        .route("/app/remove", post(remove_application))
        .route("/app/get", post(get_application))
        .route("/lb/add", post(add_lb));

    // run it
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    info!("admin server listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = shutdown.changed() => {
                  info!("admin server shutdown");
                },
            }
        })
        .await?;

    Ok(())
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

#[derive(Deserialize, Serialize)]
struct Application {
    app_id: String,
    limit_interval_seconds: u64,
    limit: u32,
}

#[derive(Deserialize, Serialize)]
struct GatewayLb {
    name: String,
    match_rule: GatewayLbMatchRule,
    rewrite: Option<GatewayRewrite>,
    service_discovery: String,
    static_upstream: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize)]
struct GatewayLbMatchRule {
    typ: String,
    value: String,
}

#[derive(Deserialize, Serialize)]
struct GatewayRewrite {
    regex: String,
    replacement: String,
}

async fn add_lb(Json(req): Json<GatewayLb>) -> &'static str {
    {
        let routes = store::routes().read().await;

        if routes.contains_key(&req.name) {
            return "lb already exists";
        }
    }

    let match_rule = match req.match_rule.typ.as_str() {
        "path_start_with" => GatewayMatchRule::PathStartsWith(req.match_rule.value),
        "path_regex" => {
            let regex = regex::Regex::new(&req.match_rule.value).unwrap();
            GatewayMatchRule::PathStartsWith(regex.to_string())
        }
        _ => return "unsupported match rule",
    };

    let rewrite = if let Some(rewrite) = req.rewrite {
        let regex = regex::Regex::new(&rewrite.regex).unwrap();
        Some((regex, rewrite.replacement))
    } else {
        None
    };

    let (service_discovery, default_health_check): (PingoraServiceDiscovery, bool) = match req
        .service_discovery
        .as_str()
    {
        "static" => (
            pingora::lb::discovery::Static::try_from_iter(req.static_upstream.unwrap_or_default())
                .unwrap(),
            false,
        ),
        "docker" => {
            // start docker background service
            let service = DockerBackgroundService::new(docker_client());
            let _ = crate::store::globalbackground_cmd(crate::store::GlobalBackgroundCmd::Add(
                DOCKER_BACKGROUND_SERVICE_NAME.to_string(),
                Box::new(Arc::new(service)),
            ))
            .await;

            (
                Box::new(DockerServiceDiscovery::new(&req.name, docker_client())),
                true,
            )
        }
        _ => return "unsupported service discovery",
    };

    let mut options =
        GatewayLoadBalancerOptions::new(match_rule, service_discovery, default_health_check);

    if let Some((regex, replacement)) = rewrite {
        options = options.with_rewrite(regex, replacement);
    }

    if let Err(e) = crate::store::proxy_cmd(ProxyCmd::Add(req.name.to_string(), options)).await {
        error!("err: {:?}", e);
    }

    "LB added"
}

async fn get_application(Json(app): Json<Application>) -> &'static str {
    let app_name = app.app_id;

    let limiter_guard = store::rate_limiters().read().await;
    if !limiter_guard.contains_key(&app_name) {
        return "Application not found";
    }
    let rate_limiter = limiter_guard.get(&app_name).unwrap();
    let limit = rate_limiter.max_req_per_second();
    let rate = rate_limiter.rate(&app_name);

    info!(
        "Application: {}, Limit: {}, Rate: {}",
        app_name, limit, rate
    );

    "Application retrieved"
}

async fn remove_application(Json(app): Json<Application>) -> &'static str {
    let app_name = app.app_id;
    store::rate_limiters().write().await.remove(&app_name);

    "Application removed"
}

async fn update_application(Json(app): Json<Application>) -> &'static str {
    let app_name = app.app_id;

    {
        let t = store::rate_limiters().read().await;
        if !t.contains_key(&app_name) {
            return "Application not found";
        }
    }

    store::rate_limiters().write().await.insert(
        app_name.clone(),
        Arc::new(RateLimiter::new(
            Rate::new(Duration::from_secs(app.limit_interval_seconds)),
            app.limit,
        )),
    );

    "Application updated"
}

async fn add_application(Json(app): Json<Application>) -> &'static str {
    let app_name = app.app_id;

    {
        let t = store::rate_limiters().read().await;
        if t.contains_key(&app_name) {
            return "Application already exists";
        }
    }

    store::rate_limiters().write().await.insert(
        app_name.clone(),
        Arc::new(RateLimiter::new(
            Rate::new(Duration::from_secs(app.limit_interval_seconds)),
            app.limit,
        )),
    );

    "Application added"
}
