use std::collections::{BTreeSet, HashMap};

use async_trait::async_trait;
use bollard::container::ListContainersOptions;
use pingora::lb::{discovery::ServiceDiscovery, Backend};
use pingora::prelude::*;

pub struct DockerServiceDiscovery {
    name: String,
    client: bollard::Docker,
}

impl DockerServiceDiscovery {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            client: bollard::Docker::connect_with_defaults().unwrap(),
        }
    }

    async fn filter_container_list(&self) -> anyhow::Result<Vec<Container>> {
        let mut filters = HashMap::new();
        filters.insert(
            "label".to_string(),
            vec![format!("com.docker.compose.service={}", self.name)],
        );

        let options = Some(ListContainersOptions {
            all: true,
            filters: filters,
            ..Default::default()
        });

        let containers = self.client.list_containers(options).await?;

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
                        name: n.ip.clone().unwrap_or_default(),
                        ip: n.ip.clone().unwrap_or_default(),
                        private_port: n.private_port,
                        public_port: n.public_port.unwrap_or_default(),
                    })
                    .collect();
                Some(Container {
                    id: id.clone(),
                    name: name.clone(),
                    ports,
                })
            })
            .collect())
    }
}

struct Container {
    id: String,
    name: String,
    ports: Vec<ContainerPort>,
}

struct ContainerPort {
    name: String,
    ip: String,
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
                backend
                    .push(Backend::new(&format!("{}:{}", "127.0.0.1", port.public_port)).unwrap());
            }
        }

        upstreams.extend(backend);
        // no readiness
        let health = HashMap::new();
        println!("discover: {:?} {:?}", upstreams, health);
        Ok((upstreams, health))
    }
}
