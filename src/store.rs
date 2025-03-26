use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
};

use tokio::sync::{OnceCell, RwLock};

use crate::{
    lb::{GatewayLoadBalancer, GatewayMatchRule},
    service::PingoraBackgroundService,
};

static PROXY_CMD: OnceCell<tokio::sync::mpsc::Sender<ProxyCmd>> = OnceCell::const_new();
pub static ROUTES: LazyLock<RwLock<HashMap<String, Arc<GatewayLoadBalancer>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static GLOBALBACKGROUND_CMD: OnceCell<tokio::sync::mpsc::Sender<GlobalBackgroundCmd>> =
    OnceCell::const_new();

pub async fn proxy_cmd(cmd: ProxyCmd) -> anyhow::Result<()> {
    Ok(PROXY_CMD
        .get()
        .ok_or(anyhow::anyhow!("PROXY_CMD not initialized"))?
        .send(cmd)
        .await?)
}

pub fn init_proxy_cmd(tx: tokio::sync::mpsc::Sender<ProxyCmd>) {
    println!("Initializing PROXY_CMD");
    match PROXY_CMD.set(tx) {
        Ok(_) => println!("PROXY_CMD initialized successfully"),
        Err(_) => println!("PROXY_CMD was already initialized!"),
    }
}

pub enum ProxyCmd {
    Add(String, GatewayMatchRule),
    Remove(String),
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

pub enum GlobalBackgroundCmd {
    Add(String, PingoraBackgroundService),
    Remove(String),
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
