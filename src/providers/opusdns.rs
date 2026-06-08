use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const DEFAULT_BASE_URL: &str = "https://api.opusdns.com";
const DEFAULT_TTL: u64 = 60;

pub struct Opusdns {
    api_key: String,
    base_url: String,
    ttl: u64,
}

impl DnsProvider for Opusdns {
    fn slug() -> &'static str {
        "opusdns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["OPUSDNS_API_Key", "OPUSDNS_API_Endpoint", "OPUSDNS_TTL"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("OPUSDNS_API_Key")
            .ok_or_else(|| Error::Config("OPUSDNS_API_Key required".into()))?
            .clone();
        let base_url = env.get("OPUSDNS_API_Endpoint")
            .map(|s| s.trim_end_matches('/').to_string())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        let ttl = env.get("OPUSDNS_TTL")
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_TTL);
        Ok(Box::new(Opusdns { api_key, base_url, ttl }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone, record_name) = self.resolve_zone(name)?;

        let body = serde_json::json!({
            "ops": [{
                "op": "upsert",
                "record": {
                    "name": record_name,
                    "type": "TXT",
                    "ttl": self.ttl,
                    "rdata": format!("\"{}\"", value),
                }
            }]
        });
        let url = format!("{}/v1/dns/{}/records", self.base_url, zone);
        let resp = http::patch(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("X-Api-Key", &self.api_key)])
            .map_err(|e| Error::Provider(format!("opusdns add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("opusdns add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone, record_name) = match self.resolve_zone(name) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let body = serde_json::json!({
            "ops": [{
                "op": "remove",
                "record": {
                    "name": record_name,
                    "type": "TXT",
                    "ttl": self.ttl,
                    "rdata": format!("\"{}\"", value),
                }
            }]
        });
        let url = format!("{}/v1/dns/{}/records", self.base_url, zone);
        let resp = match http::patch(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("X-Api-Key", &self.api_key)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if resp.status >= 400 {
            eprintln!("warning: opusdns remove TXT: HTTP {} {}", resp.status, resp.body);
        }
        Ok(())
    }
}

impl Opusdns {
    fn resolve_zone(&self, fqdn: &str) -> Result<(String, String), Error> {
        let domain = fqdn.trim_end_matches('.');
        let parts: Vec<&str> = domain.split('.').collect();

        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            let url = format!("{}/v1/dns/{}", self.base_url, h);
            if let Ok(resp) = http::get(&url, &[("X-Api-Key", &self.api_key)]) {
                if resp.body.contains("\"dnssec_status\"") {
                    let record_name = if i == 0 {
                        "@".to_string()
                    } else {
                        parts[..i].join(".")
                    };
                    return Ok((h, record_name));
                }
            }
        }
        Err(Error::Provider(format!("opusdns: no valid zone found for: {domain}")))
    }
}
