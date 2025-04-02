use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
};

use bollard::secret::ContainerSummary;
use tokio::sync::{OnceCell, RwLock};

use crate::{
    lb::GatewayLoadBalancer, proxy::ProxyCmd, rate_limit::RateLimiter, service::GlobalBackgroundCmd,
};

static PROXY_CMD: OnceCell<tokio::sync::mpsc::Sender<ProxyCmd>> = OnceCell::const_new();
static ROUTES: LazyLock<RwLock<HashMap<String, Arc<GatewayLoadBalancer>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static GLOBALBACKGROUND_CMD: OnceCell<tokio::sync::mpsc::Sender<GlobalBackgroundCmd>> =
    OnceCell::const_new();
static CONTAINERS: LazyLock<RwLock<Vec<ContainerSummary>>> =
    LazyLock::new(|| RwLock::new(Vec::new()));
static APPLICATIONS: LazyLock<RwLock<HashMap<String, Arc<GatewayApplication>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static DOCKER_CLIENT: LazyLock<Arc<bollard::Docker>> = LazyLock::new(|| {
    Arc::new(bollard::Docker::connect_with_defaults().expect("fail to connect to docker"))
});
static CONFIG: OnceCell<crate::config::GatewayConfig> = OnceCell::const_new();

pub fn docker_client() -> Arc<bollard::Docker> {
    Arc::clone(&DOCKER_CLIENT)
}

pub fn containers() -> &'static RwLock<Vec<ContainerSummary>> {
    &CONTAINERS
}

pub fn applications() -> &'static RwLock<HashMap<String, Arc<GatewayApplication>>> {
    &APPLICATIONS
}

pub fn routes() -> &'static RwLock<HashMap<String, Arc<GatewayLoadBalancer>>> {
    &ROUTES
}

pub async fn proxy_cmd(cmd: ProxyCmd) -> anyhow::Result<()> {
    Ok(PROXY_CMD
        .get()
        .ok_or(anyhow::anyhow!("PROXY_CMD not initialized"))?
        .send(cmd)
        .await?)
}

pub fn init_proxy_cmd(tx: tokio::sync::mpsc::Sender<ProxyCmd>) {
    PROXY_CMD
        .set(tx)
        .expect("expected PROXY_CMD to be set only once");
}

pub async fn globalbackground_cmd(cmd: GlobalBackgroundCmd) -> anyhow::Result<()> {
    Ok(GLOBALBACKGROUND_CMD
        .get()
        .ok_or(anyhow::anyhow!("GLOBALBACKGROUND_CMD not initialized"))?
        .send(cmd)
        .await?)
}

pub fn init_globalbackground_cmd(tx: tokio::sync::mpsc::Sender<GlobalBackgroundCmd>) {
    GLOBALBACKGROUND_CMD
        .set(tx)
        .expect("expected GLOBALBACKGROUND_CMD to be set only once");
}

pub fn config() -> &'static crate::config::GatewayConfig {
    CONFIG.get().expect("CONFIG not initialized")
}

pub fn init_config(config: crate::config::GatewayConfig) {
    CONFIG
        .set(config)
        .expect("expected CONFIG to be set only once");
}

pub struct GatewayApplication {
    rate_limiter: RateLimiter,
}

impl GatewayApplication {
    pub fn new(rate_limiter: RateLimiter) -> Self {
        Self { rate_limiter }
    }

    pub fn rate_limiter(&self) -> &RateLimiter {
        &self.rate_limiter
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use tokio::sync::RwLock;

    #[tokio::test]
    async fn test_rwlock() {
        let lock = Arc::new(RwLock::new(1));
        let c_lock = lock.clone();

        let n = lock.read().await;
        assert_eq!(*n, 1);

        tokio::spawn(async move {
            // While main has an active read lock, we acquire one too.
            let r = c_lock.read().await;
            assert_eq!(*r, 1);
        })
        .await
        .expect("The spawned task has panicked");

        // Drop the guard after the spawned task finishes.
        drop(n);
    }
}
