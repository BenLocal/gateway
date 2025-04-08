use axum::{
    response::Html,
    routing::{get, post},
    Json, Router,
};
use pingora::server::ShutdownWatch;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{
    app::{self, Application, LbInfo, LbMatchRuleInfo, LbRewriteInfo},
    store,
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
struct ApplicationRequest {
    app_id: String,
    limit_interval_seconds: u64,
    limit: u32,
}

impl Into<Application> for ApplicationRequest {
    fn into(self) -> Application {
        Application {
            app_id: self.app_id,
            limit_interval_seconds: self.limit_interval_seconds,
            limit: self.limit,
        }
    }
}

#[derive(Deserialize, Serialize)]
struct GatewayLbRequest {
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

impl Into<LbInfo> for GatewayLbRequest {
    fn into(self) -> LbInfo {
        LbInfo {
            name: self.name,
            match_rule: LbMatchRuleInfo {
                typ: self.match_rule.typ,
                value: self.match_rule.value,
            },
            rewrite: self.rewrite.map(|r| LbRewriteInfo {
                regex: r.regex,
                replacement: r.replacement,
            }),
            service_discovery: self.service_discovery,
            upstream: self.static_upstream,
        }
    }
}

async fn add_lb(Json(req): Json<GatewayLbRequest>) -> &'static str {
    {
        let routes = store::routes().read().await;

        if routes.contains_key(&req.name) {
            return "lb already exists";
        }
    }

    app::add_load_balancer(req.into()).await;

    "LB added"
}

async fn get_application(Json(app): Json<ApplicationRequest>) -> &'static str {
    let app_name = app.app_id;

    let apps = store::applications().read().await;
    if !apps.contains_key(&app_name) {
        return "Application not found";
    }
    let app = apps.get(&app_name).unwrap();
    let limit = app.rate_limiter().max_req_per_second();
    let rate = app.rate_limiter().rate(&app_name);

    info!(
        "Application: {}, Limit: {}, Rate: {}",
        app_name, limit, rate
    );

    "Application retrieved"
}

async fn remove_application(Json(app): Json<ApplicationRequest>) -> &'static str {
    let app_name = app.app_id;
    store::applications().write().await.remove(&app_name);

    "Application removed"
}

async fn update_application(Json(app): Json<ApplicationRequest>) -> &'static str {
    let app_name = app.app_id.clone();

    {
        let t = store::applications().read().await;
        if !t.contains_key(&app_name) {
            return "Application not found";
        }
    }

    app::add_application(app.into()).await;

    "Application updated"
}

async fn add_application(Json(app): Json<ApplicationRequest>) -> &'static str {
    let app_name = app.app_id.clone();

    {
        let t = store::applications().read().await;
        if t.contains_key(&app_name) {
            return "Application already exists";
        }
    }

    app::add_application(app.into()).await;

    "Application added"
}
