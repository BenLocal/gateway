use std::str::FromStr;

use async_trait::async_trait;
use axum::http::{uri::PathAndQuery, Uri};
use pingora::{
    prelude::*,
    proxy::{ProxyHttp, Session},
};

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

            let lb = match reoutes.iter().find_map(|(_, lb)| {
                if lb.matches_path(path) {
                    Some(lb)
                } else {
                    None
                }
            }) {
                Some(lb) => lb,
                None => return Err(Error::new(ErrorType::ConnectNoRoute)),
            };

            if let Some(new_path) = lb.rewrite_path(path) {
                let req = session.req_header_mut();
                let mut uri = req.uri.clone().into_parts();
                uri.path_and_query = uri.path_and_query.map(|pq| {
                    let query = pq.query();
                    let path_and_query = match query {
                        Some(query) => format!("{}?{}", new_path, query),
                        None => new_path.to_string(),
                    };
                    PathAndQuery::from_str(&path_and_query).unwrap()
                });
                req.set_uri(Uri::from_parts(uri).unwrap());
            }

            match lb.lb().select_with(b"", 256, |backend, health| {
                if backend.ext.is_empty() || backend.ext.get::<u64>() == Some(&1) {
                    return health;
                }
                return false;
            }) {
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
