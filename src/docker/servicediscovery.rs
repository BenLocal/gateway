use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use async_trait::async_trait;
use bollard::network::InspectNetworkOptions;
use bollard::secret::ContainerSummary;
use pingora::lb::{discovery::ServiceDiscovery, Backend};
use pingora::prelude::*;
use tracing::error;

use crate::r#const::{
    DOCKER_LABEL_DOCKER_COMPOSE_SERVICE, DOCKER_LABEL_GATEWAY_HOST_IP, DOCKER_LABEL_GATEWAY_MODE,
};
use crate::store;

pub struct DockerServiceDiscovery {
    name: String,
    client: Arc<bollard::Docker>,
}

impl DockerServiceDiscovery {
    pub fn new(name: &str, client: Arc<bollard::Docker>) -> Self {
        Self {
            name: name.to_string(),
            client: client,
        }
    }

    async fn filter_container_list(&self) -> anyhow::Result<Vec<Container>> {
        let containers = store::containers()
            .read()
            .await
            .iter()
            .filter(|c| {
                c.labels
                    .as_ref()
                    .map(|labels| {
                        labels
                            .get(DOCKER_LABEL_DOCKER_COMPOSE_SERVICE)
                            .map(|s| s == &self.name)
                            .unwrap_or(false)
                    })
                    .unwrap_or(false)
            })
            .cloned()
            .collect::<Vec<_>>();

        let docker0_ip = self.get_docker0_ip().await;
        Ok(containers
            .iter()
            .filter_map(|c| {
                let id = c.id.clone()?;
                let name = c.names.clone()?.get(0)?.to_string();
                let ports = c
                    .ports
                    .clone()?
                    .iter()
                    .map(|n| ContainerPort {
                        private_port: n.private_port,
                        public_port: n.public_port.unwrap_or_default(),
                    })
                    .collect();

                Some(Container {
                    _id: id.clone(),
                    _name: name.clone(),
                    ports,
                    host_ip: self.get_host_ip(&c, &docker0_ip),
                    mode: self.get_container_mode(&c),
                    inner_ips: self.get_bridge_ips(&c),
                })
            })
            .collect())
    }

    fn get_host_ip(&self, container: &ContainerSummary, docker0: &str) -> String {
        container
            .labels
            .as_ref()
            .map(|labels| {
                labels
                    .get(DOCKER_LABEL_GATEWAY_HOST_IP)
                    .map(|ip| ip.to_string())
            })
            .flatten()
            .unwrap_or(docker0.to_string())
    }

    fn get_bridge_ips(&self, container: &ContainerSummary) -> Option<HashMap<String, String>> {
        container.network_settings.as_ref().and_then(|ns| {
            let networks = ns.networks.as_ref()?;
            let mut bridge_ip = HashMap::new();
            for (name, network) in networks {
                if let Some(ip) = network.ip_address.clone() {
                    bridge_ip.insert(name.clone(), ip);
                }
            }
            Some(bridge_ip)
        })
    }

    fn get_container_mode(&self, container: &ContainerSummary) -> ContainerMode {
        let label = container
            .labels
            .as_ref()
            .and_then(|labels| labels.get(DOCKER_LABEL_GATEWAY_MODE))
            .map(|s| s.as_str());

        match label {
            Some("host") => ContainerMode::Host,
            Some("bridge") => ContainerMode::Bridge,
            _ => {
                // check if the container is running in host mode
                if container
                    .network_settings
                    .as_ref()
                    .and_then(|ns| ns.networks.as_ref().map(|n| n.get("host")))
                    .flatten()
                    .is_some()
                {
                    ContainerMode::Host
                } else {
                    ContainerMode::Bridge
                }
            }
        }
    }

    async fn get_docker0_ip(&self) -> String {
        match self
            .client
            .inspect_network("bridge", None::<InspectNetworkOptions<String>>)
            .await
        {
            Ok(network) => {
                if let Some(ipam) = network.ipam {
                    if let Some(config) = ipam.config {
                        if !config.is_empty() {
                            if let Some(gateway) = &config[0].gateway {
                                return gateway.clone();
                            }
                        }
                    }
                }
                // 如果无法从API获取，返回默认IP
                "172.17.0.1".to_string()
            }
            Err(e) => {
                error!("Failed to inspect docker bridge network: {}", e);
                // 出错时返回默认IP
                "172.17.0.1".to_string()
            }
        }
    }
}

enum ContainerMode {
    Host,
    Bridge,
}

struct Container {
    _id: String,
    _name: String,
    host_ip: String,
    inner_ips: Option<HashMap<String, String>>,
    mode: ContainerMode,
    ports: Vec<ContainerPort>,
}

struct ContainerPort {
    private_port: u16,
    public_port: u16,
}

#[async_trait]
impl ServiceDiscovery for DockerServiceDiscovery {
    async fn discover(&self) -> Result<(BTreeSet<Backend>, HashMap<u64, bool>)> {
        let mut upstreams = BTreeSet::new();
        let mut backend = vec![];
        let containers = self.filter_container_list().await.unwrap();

        for container in containers {
            if let Some(port) = container.ports.get(0) {
                let ip_prots = match container.mode {
                    ContainerMode::Host => Some(vec![(container.host_ip, port.public_port)]),
                    ContainerMode::Bridge => {
                        if let Some(ip) = container.inner_ips {
                            Some(
                                ip.into_iter()
                                    .map(|(_, ip)| (ip, port.private_port))
                                    .collect::<Vec<_>>(),
                            )
                        } else {
                            None
                        }
                    }
                };

                if let Some(v) = ip_prots {
                    for (ip, port) in v {
                        backend.push(Backend::new(&format!("{}:{}", ip, port)).unwrap());
                    }
                }
            }
        }

        upstreams.extend(backend);
        // no readiness
        let health = HashMap::new();
        Ok((upstreams, health))
    }
}
