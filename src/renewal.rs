use serde_json::{Value, json};

use crate::config::{self, Config};
use crate::error::Error;

#[derive(Clone, Debug)]
pub struct RenewalConfig {
    pub domains: Vec<String>,
    pub provider: String,
    pub email: String,
    pub server: String,
    pub staging: bool,
    pub challenge: String,
    pub key_type: String,
    pub dnssleep: u64,
    pub dns_servers: Vec<String>,
    pub dns_timeout: u64,
    pub preferred_chain: Option<String>,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    pub fullchain_path: Option<String>,
    pub chain_path: Option<String>,
    pub reloadcmd: Option<String>,
    pub deploy_hook: Option<String>,
    pub pre_hook: Option<String>,
    pub post_hook: Option<String>,
}

impl RenewalConfig {
    pub fn new(domains: Vec<String>, provider: String, email: String, server: String) -> Self {
        RenewalConfig {
            domains,
            provider,
            email,
            server,
            staging: false,
            challenge: "dns-01".to_string(),
            key_type: "ecdsa-p256".to_string(),
            dnssleep: 10,
            dns_servers: Vec::new(),
            dns_timeout: 120,
            preferred_chain: None,
            cert_path: None,
            key_path: None,
            fullchain_path: None,
            chain_path: None,
            reloadcmd: None,
            deploy_hook: None,
            pre_hook: None,
            post_hook: None,
        }
    }

    pub fn main_domain(&self) -> &str {
        self.domains.first().map(|s| s.as_str()).unwrap_or("")
    }

    pub fn load(config: &Config, domain: &str) -> Result<Self, Error> {
        let path = config.renewal_file(domain);
        let data = std::fs::read_to_string(&path)?;
        let v: Value = serde_json::from_str(&data)
            .map_err(|e| Error::Config(format!("parse renewal config: {e}")))?;
        Ok(Self::from_value(&v, domain))
    }

    pub fn from_value(v: &Value, fallback_domain: &str) -> Self {
        let mut domains = string_array(v, "domains");
        if domains.is_empty() {
            domains.push(fallback_domain.to_string());
        }
        RenewalConfig {
            domains,
            provider: string(v, "provider"),
            email: string(v, "email"),
            server: string(v, "server"),
            staging: boolean(v, "staging")
                .or_else(|| boolean(v, "test"))
                .unwrap_or(false),
            challenge: string_or(v, "challenge", "dns-01"),
            key_type: string_or(v, "key_type", "ecdsa-p256"),
            dnssleep: unsigned(v, "dnssleep").unwrap_or(10),
            dns_servers: string_array(v, "dns_servers"),
            dns_timeout: unsigned(v, "dns_timeout").unwrap_or(120),
            preferred_chain: optional_string(v, "preferred_chain"),
            cert_path: optional_string(v, "cert_path"),
            key_path: optional_string(v, "key_path"),
            fullchain_path: optional_string(v, "fullchain_path"),
            chain_path: optional_string(v, "chain_path"),
            reloadcmd: optional_string(v, "reloadcmd"),
            deploy_hook: optional_string(v, "deploy_hook"),
            pre_hook: optional_string(v, "pre_hook"),
            post_hook: optional_string(v, "post_hook"),
        }
    }

    pub fn to_value(&self) -> Value {
        json!({
            "domains": self.domains,
            "provider": self.provider,
            "email": self.email,
            "server": self.server,
            "staging": self.staging,
            "challenge": self.challenge,
            "key_type": self.key_type,
            "dnssleep": self.dnssleep,
            "dns_servers": self.dns_servers,
            "dns_timeout": self.dns_timeout,
            "preferred_chain": self.preferred_chain,
            "cert_path": self.cert_path,
            "key_path": self.key_path,
            "fullchain_path": self.fullchain_path,
            "chain_path": self.chain_path,
            "reloadcmd": self.reloadcmd,
            "deploy_hook": self.deploy_hook,
            "pre_hook": self.pre_hook,
            "post_hook": self.post_hook,
        })
    }

    pub fn save(&self, config: &Config) -> Result<(), Error> {
        let path = config.renewal_file(self.main_domain());
        let body = serde_json::to_string_pretty(&self.to_value()).unwrap();
        config::write_atomic(&path, body.as_bytes(), 0o644)
    }
}

fn string(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string()
}

fn string_or(v: &Value, key: &str, default: &str) -> String {
    v.get(key)
        .and_then(|s| s.as_str())
        .unwrap_or(default)
        .to_string()
}

fn optional_string(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

fn boolean(v: &Value, key: &str) -> Option<bool> {
    v.get(key).and_then(|b| b.as_bool())
}

fn unsigned(v: &Value, key: &str) -> Option<u64> {
    v.get(key).and_then(|n| n.as_u64())
}

fn string_array(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s.as_str())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}
