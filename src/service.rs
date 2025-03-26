use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use pingora::{
    server::{ListenFds, ShutdownWatch},
    services::{background::BackgroundService, Service},
};

use crate::{lb::GatewayLoadBalancer, store::GlobalBackgroundCmd};

pub type PingoraBackgroundService = Box<Arc<dyn BackgroundService + Send + Sync + 'static>>;

struct BackgroundServiceInner {
    inner: Arc<PingoraBackgroundService>,
    closer: Option<tokio::sync::watch::Sender<bool>>,
}

impl BackgroundServiceInner {
    fn set_close(&mut self, closer: tokio::sync::watch::Sender<bool>) {
        self.closer = Some(closer);
    }

    fn task(&self) -> Arc<PingoraBackgroundService> {
        self.inner.clone()
    }
}

pub struct GlobalBackgroundService {
    services: HashMap<String, BackgroundServiceInner>,
    cmd_rev: tokio::sync::mpsc::Receiver<GlobalBackgroundCmd>,
}

impl GlobalBackgroundService {
    pub fn new() -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        crate::store::init_globalbackground_cmd(tx);
        Self {
            cmd_rev: rx,
            services: HashMap::new(),
        }
    }
}

#[async_trait]
impl Service for GlobalBackgroundService {
    async fn start_service(&mut self, _fds: Option<ListenFds>, shutdown: ShutdownWatch) {
        for (_, hc) in self.services.iter_mut() {
            let (hc_tx, hc_rx) = tokio::sync::watch::channel(false);
            hc.set_close(hc_tx);
            let task = hc.task();
            tokio::spawn(async move {
                task.start(hc_rx).await;
            });
        }

        loop {
            let mut shutdown = shutdown.clone();
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        break;
                    }
                }
                Some(v) = self.cmd_rev.recv() => {
                    match v {
                        GlobalBackgroundCmd::Add(key, hc) => {
                            let (hc_tx, hc_rx) = tokio::sync::watch::channel(false);
                            let hc = Arc::new(hc);
                            let hc_clone = hc.clone();
                            tokio::spawn(async move {
                                hc_clone.start(hc_rx).await;
                            });
                            self.services.insert(
                                key,
                                BackgroundServiceInner {
                                    inner: hc,
                                    closer: Some(hc_tx),
                                },
                            );
                        }
                        GlobalBackgroundCmd::Remove(key) => {
                            if let Some(hc) = self.services.remove(&key) {
                                if let Some(closer) = hc.closer {
                                    let _ = closer.send(true);
                                }
                            }
                        }
                    }
                }
            }
        }

        for (_, hc) in self.services.iter() {
            if let Some(closer) = &hc.closer {
                let _ = closer.send(true);
            }
        }
    }

    fn name(&self) -> &str {
        "GlobalBackgroundService"
    }

    fn threads(&self) -> Option<usize> {
        Some(1)
    }
}

pub struct ProxyService {
    cmd_rev: tokio::sync::mpsc::Receiver<crate::store::ProxyCmd>,
}

impl ProxyService {
    pub fn new() -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        crate::store::init_proxy_cmd(tx);

        Self { cmd_rev: rx }
    }
}

#[async_trait]
impl Service for ProxyService {
    async fn start_service(&mut self, _fds: Option<ListenFds>, shutdown: ShutdownWatch) {
        loop {
            let mut shutdown = shutdown.clone();
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        break;
                    }
                }
                Some(v) = self.cmd_rev.recv() => {
                    match v {
                        crate::store::ProxyCmd::Add(key, rule) => {
                            let lb = Arc::new(GatewayLoadBalancer::new(&key, rule));
                            let mut reoutes = crate::store::ROUTES.write().await;
                            reoutes.insert(key.to_string(), Arc::clone(&lb));

                            let _ = crate::store::globalbackground_cmd(crate::store::GlobalBackgroundCmd::Add(
                                format!("{}_hc", key),
                                Box::new(lb),
                            )).await;
                            println!("add route: {}", key);
                        }
                        crate::store::ProxyCmd::Remove(key) => {
                            let mut reoutes = crate::store::ROUTES.write().await;
                            reoutes.remove(&key);
                            let _ = crate::store::globalbackground_cmd(crate::store::GlobalBackgroundCmd::Remove(
                                format!("{}_hc", key)
                            )).await;
                            println!("remove route: {}", key);
                        }
                    }
                }
            }
        }
    }

    fn name(&self) -> &str {
        "ProxyService"
    }

    fn threads(&self) -> Option<usize> {
        Some(1)
    }
}
