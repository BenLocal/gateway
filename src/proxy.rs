use async_trait::async_trait;
use pingora::{
    prelude::*,
    proxy::{ProxyHttp, Session},
};

use crate::lb::GatewayMatchRule;

pub struct GatewayProxy;

impl GatewayProxy {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProxyHttp for GatewayProxy {
    type CTX = ();
    fn new_ctx(&self) -> () {
        ()
    }

    async fn upstream_peer(&self, session: &mut Session, _ctx: &mut ()) -> Result<Box<HttpPeer>> {
        let path = match session.req_header().uri.path() {
            path => path,
        };

        let upstream = {
            let reoutes = &crate::store::ROUTES.read().await;

            let lb = match reoutes.iter().find_map(|(_, lb)| match &lb.match_rule() {
                GatewayMatchRule::PathStartsWith(prefix) => {
                    if path.starts_with(prefix) {
                        Some(lb)
                    } else {
                        None
                    }
                }
            }) {
                Some(lb) => lb,
                None => return Err(Error::new(ErrorType::ConnectNoRoute)),
            };
            match lb.lb().select(b"", 256) {
                Some(upstream) => {
                    println!("upstream peer is: {} --> {:?}", lb.name(), upstream);
                    upstream
                }
                None => return Err(Error::new(ErrorType::ConnectNoRoute)),
            }
        };

        let peer = Box::new(HttpPeer::new(upstream, false, "test".to_string()));
        Ok(peer)
    }
}
