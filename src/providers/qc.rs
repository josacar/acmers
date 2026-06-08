use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.quic.cloud/v2";

pub struct Qc {
    key: String,
    email: String,
}

impl DnsProvider for Qc {
    fn slug() -> &'static str {
        "qc"
    }

    fn env_vars() -> &'static [&'static str] {
        &["QC_API_KEY", "QC_API_EMAIL"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let key = env.get("QC_API_KEY")
            .ok_or_else(|| Error::Config("QC_API_KEY required".into()))?
            .clone();
        let email = env.get("QC_API_EMAIL")
            .ok_or_else(|| Error::Config("QC_API_EMAIL required".into()))?
            .clone();
        Ok(Box::new(Qc { key, email }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = self.resolve_zone(domain)?;

        let body = serde_json::json!({
            "type": "TXT",
            "name": name,
            "content": value,
            "ttl": 1800,
        });
        let url = format!("{BASE_URL}/zones/{domain_id}/records");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("X-Auth-Email", &self.email), ("X-Auth-Key", &self.key)])
            .map_err(|e| Error::Provider(format!("qc add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("qc add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = match self.resolve_zone(domain) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };

        let list_url = format!("{BASE_URL}/zones/{domain_id}/records");
        let resp = match http::get(&list_url, &[("X-Auth-Email", &self.email), ("X-Auth-Key", &self.key)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records = v.as_array()
            .or_else(|| v.get("data").and_then(|d| d.as_array()))
            .or_else(|| v.get("records").and_then(|r| r.as_array()));
        if let Some(records) = records {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record_id(record) {
                        let del_url = format!("{BASE_URL}/zones/{domain_id}/records/{id}");
                        let _ = http::delete(&del_url, &[("X-Auth-Email", &self.email), ("X-Auth-Key", &self.key)]);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Qc {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let url = format!("{BASE_URL}/zones");
        let resp = http::get(&url, &[("X-Auth-Email", &self.email), ("X-Auth-Key", &self.key)])
            .map_err(|e| Error::Provider(format!("qc zones: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("qc zones: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Provider(format!("qc zones: {e}")))?;
        let zones = v.as_array()
            .or_else(|| v.get("data").and_then(|d| d.as_array()))
            .or_else(|| v.get("zones").and_then(|z| z.as_array()));
        if let Some(zones) = zones {
            for zone in zones {
                if let Some(zone_name) = zone.get("name").and_then(|n| n.as_str()) {
                    let stripped = zone_name.strip_suffix('.').unwrap_or(zone_name);
                    if stripped == domain {
                        return Ok(stripped.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("qc: zone not found for {domain}")))
    }
}

fn record_id(v: &Value) -> Option<String> {
    v.get("id").and_then(|i| {
        if i.is_string() { i.as_str().map(|s| s.to_string()) }
        else if i.is_u64() { i.as_u64().map(|n| n.to_string()) }
        else if i.is_i64() { i.as_i64().map(|n| n.to_string()) }
        else { None }
    })
}
