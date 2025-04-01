use std::{collections::HashSet, sync::Arc};

use async_trait::async_trait;
use bollard::{
    container::ListContainersOptions,
    network::{ConnectNetworkOptions, ListNetworksOptions},
    secret::{ContainerSummary, Network},
};
use pingora::{server::ShutdownWatch, services::background::BackgroundService};
use tracing::info;

use crate::r#const::DOCKER_LABEL_GATEWAY_CONNECT_NETWORK;

pub struct DockerBackgroundService {
    client: Arc<bollard::Docker>,
}

impl DockerBackgroundService {
    pub fn new(client: Arc<bollard::Docker>) -> Self {
        Self { client }
    }

    async fn update(&self) -> anyhow::Result<Vec<ContainerSummary>> {
        let options = Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        });
        let containers = self.client.list_containers(options).await?;
        let network_connects = containers
            .iter()
            .filter(|c| {
                c.labels
                    .as_ref()
                    .map(|l| {
                        l.get(DOCKER_LABEL_GATEWAY_CONNECT_NETWORK)
                            .map(|v| v == "true")
                    })
                    .flatten()
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();

        if !network_connects.is_empty() {
            let networks = self
                .client
                .list_networks(None::<ListNetworksOptions<String>>)
                .await?;

            for c in network_connects {
                if let Some(id) = c.id.clone() {
                    let container_networks = self.get_container_networks(c).unwrap_or_default();
                    let _ = self
                        .update_bridge_networks(&id, container_networks, &networks)
                        .await;
                }
            }
        }

        Ok(containers)
    }

    async fn update_bridge_networks(
        &self,
        container_id: &str,
        container_networks: HashSet<String>,
        networks: &Vec<Network>,
    ) -> anyhow::Result<()> {
        for network in networks {
            let name = network.name.clone().unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            match network.driver.as_ref() {
                Some(driver) if driver.as_str() == "bridge" => {
                    if !container_networks.contains(&name) {
                        // Update the container with the new network information
                        self.client
                            .connect_network(
                                &name,
                                ConnectNetworkOptions {
                                    container: container_id,
                                    ..Default::default()
                                },
                            )
                            .await?;

                        info!("Connected container {} to network {}", container_id, name);
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn get_container_networks(&self, container: &ContainerSummary) -> Option<HashSet<String>> {
        Some(
            container
                .network_settings
                .as_ref()?
                .networks
                .as_ref()?
                .keys()
                .map(|k| k.to_string())
                .collect::<HashSet<_>>(),
        )
    }
}

#[async_trait]
impl BackgroundService for DockerBackgroundService {
    async fn start(&self, shutdown: ShutdownWatch) {
        let mut shutdown = shutdown.clone();
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        break;
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {
                    if let Ok(c) = self.update().await {
                        {
                            let mut containers = crate::store::CONTAINERS.write().await;
                            containers.clear();
                            containers.extend(c);
                        }
                    }
                }
            }
        }
    }
}
