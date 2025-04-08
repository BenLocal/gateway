use admin::service::AdminService;
use app::{Application, BackgroundServer, LbInfo};
use pingora::{proxy::http_proxy_service, server::Server};
use proxy::GatewayProxy;
use service::{GlobalBackgroundService, ProxyService};
use tracing::{info, Level};

mod admin;
mod app;
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
    if let Some(bg) = &config.backgrounds {
        for b in bg {
            app::start_background_service(BackgroundServer::from(b.to_string())).await;
        }
    }

    if let Some(applications) = &config.applications {
        for app in applications {
            app::add_application(Application::from(app)).await;
        }
    }

    if let Some(load_balancers) = &config.load_balancers {
        for lb in load_balancers {
            app::add_load_balancer(LbInfo::from(lb)).await;
        }
    }
}
