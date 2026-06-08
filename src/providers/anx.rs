use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Anx {
    token: String,
}

impl DnsProvider for Anx {
    fn slug() -> &'static str {
        "anx"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ANX_Token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("ANX_Token")
            .ok_or_else(|| Error::Config("ANX_Token required".into()))?.clone();
        Ok(Box::new(Anx { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = self.resolve_zone(domain)?;
        let url = format!("https://engine.anexia-it.com/api/clouddns/v1/zone.json/{zone}/records");
        let body = serde_json::json!({
            "name": name,
            "type": "TXT",
            "rdata": value,
        });
        let auth = format!("Token {}", self.token);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("anx add TXT: {e}")))?;
        if resp.status >= 300 {
            return Err(Error::Provider(format!("anx add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = match self.resolve_zone(domain) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let auth = format!("Token {}", self.token);
        let url = format!("https://engine.anexia-it.com/api/clouddns/v1/zone.json/{zone}/records?name={name}&type=TXT");
        let resp = match http::get(&url, &[("Authorization", &auth)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(records) = v.as_array() {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("rdata").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("identifier").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://engine.anexia-it.com/api/clouddns/v1/zone.json/{zone}/records/{id}");
                        let _ = http::delete(&del_url, &[("Authorization", &auth)]);
                    }
                }
            }
        }
        Ok(())
    }
}

impl Anx {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let auth = format!("Token {}", self.token);
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len() {
            let candidate = parts[i..].join(".");
            let url = format!("https://engine.anexia-it.com/api/clouddns/v1/zone.json/{candidate}");
            if let Ok(resp) = http::get(&url, &[("Authorization", &auth)]) {
                if let Ok(v) = serde_json::from_str::<Value>(&resp.body) {
                    if v.get("name").and_then(|n| n.as_str()) == Some(candidate.as_str()) {
                        return Ok(candidate);
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
