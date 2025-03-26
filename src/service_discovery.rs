use std::collections::{BTreeSet, HashMap};

use async_trait::async_trait;
use pingora::lb::{discovery::ServiceDiscovery, Backend};
use pingora::prelude::*;

// pub struct GatewayServiceDiscovery;

// impl GatewayServiceDiscovery {
//     pub fn new() -> Self {
//         Self {}
//     }
// }

// #[async_trait]
// impl ServiceDiscovery for GatewayServiceDiscovery {
//     async fn discover(&self) -> Result<(BTreeSet<Backend>, HashMap<u64, bool>)> {
//         let mut upstreams = BTreeSet::new();
//         upstreams.extend(vec![Backend::new("127.0.0.1:3000").unwrap()]);
//         // no readiness
//         let health = HashMap::new();
//         println!("discover: {:?} {:?}", upstreams, health);
//         Ok((upstreams, health))
//     }
// }
