use std::{fs::File, io::BufReader};

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct GatewayConfig {
    pub backgrounds: Option<Vec<String>>,
    pub applications: Option<Vec<GatewayApplicationConfig>>,
    pub load_balancers: Option<Vec<GatewayLoadBalancerConfig>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GatewayApplicationConfig {
    pub app_id: String,
    pub limit_interval_seconds: u64,
    pub limit: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GatewayLoadBalancerConfig {
    pub name: String,
    pub match_rule: GatewayLoadBalancerMatchRuleConfig,
    pub rewrite: Option<GatewayRewriteConfig>,
    pub service_discovery: String,
    pub upstream: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GatewayLoadBalancerMatchRuleConfig {
    /// The type of match rule, e.g., "path_start_with" or "path_regex"
    #[serde(rename = "type")]
    pub typ: String,
    pub value: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GatewayRewriteConfig {
    pub regex: String,
    pub replacement: String,
}

impl GatewayConfig {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let file = File::open(path)?;
        let config: GatewayConfig = serde_yaml::from_reader(BufReader::new(file))?;
        Ok(config)
    }
}
