use std::str::FromStr;

use async_trait::async_trait;
use axum::http::{uri::PathAndQuery, Uri};
use pingora::{
    prelude::*,
    proxy::{ProxyHttp, Session},
};
use tracing::info;

use crate::r#const::{GATEWAY_HEADER_EXT, GATEWAY_QUERY_EXT};

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

            let ext = get_ext_value(session);

            match lb.lb().select_with(b"", 256, |backend, health| {
                if backend.ext.is_empty() {
                    return health;
                }

                // Check if the backend has a label for the extension
                // and if it matches the extension from the request
                // If the backend has a label for the extension, check if it matches
                // the extension from the request
                if let Some(ext) = ext.as_ref() {
                    if let Some(lbext) = backend.ext.get::<String>() {
                        if lbext == ext {
                            return health;
                        }
                    }
                }

                return false;
            }) {
                Some(upstream) => {
                    info!("upstream peer is: {} --> {:?}", lb.name(), upstream);
                    upstream
                }
                None => return Err(Error::new(ErrorType::ConnectNoRoute)),
            }
        };

        let peer = Box::new(HttpPeer::new(upstream, false, "test".to_string()));
        Ok(peer)
    }
}

fn get_ext_value(session: &Session) -> Option<String> {
    let ext = session.get_header(GATEWAY_HEADER_EXT);
    if let Some(v) = ext {
        if let Ok(v) = v.to_str() {
            return Some(v.to_string());
        }
    }

    let query = session.req_header().uri.query();
    if let Some(query_str) = query {
        if let Some(value) = get_query_param(query_str, GATEWAY_QUERY_EXT) {
            return Some(value);
        }
    }

    None
}

fn get_query_param(query_str: &str, param_name: &str) -> Option<String> {
    form_urlencoded::parse(query_str.as_bytes())
        .find(|(key, _)| key == param_name)
        .map(|(_, value)| value.into_owned())
}
