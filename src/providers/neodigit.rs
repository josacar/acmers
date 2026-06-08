use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.neodigit.net/v1";

pub struct Neodigit {
    token: String,
}

impl DnsProvider for Neodigit {
    fn slug() -> &'static str {
        "neodigit"
    }

    fn env_vars() -> &'static [&'static str] {
        &["NEODIGIT_API_TOKEN"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("NEODIGIT_API_TOKEN")
            .ok_or_else(|| Error::Config("NEODIGIT_API_TOKEN required".into()))?
            .clone();
        Ok(Box::new(Neodigit { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone_id, sub_domain) = self.get_root(domain, name)?;

        let body = serde_json::json!({
            "record": {
                "type": "TXT",
                "name": sub_domain,
                "content": value,
                "ttl": 60,
            }
        });
        let url = format!("{BASE_URL}/dns/zones/{zone_id}/records");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("X-TCPanel-Token", &self.token)])
            .map_err(|e| Error::Provider(format!("neodigit add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("neodigit add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone_id, _sub_domain) = match self.get_root(domain, name) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let list_url = format!("{BASE_URL}/dns/zones/{zone_id}/records?type=TXT&name={name}&content={value}");
        let resp = match http::get(&list_url, &[("X-TCPanel-Token", &self.token)]) {
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
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT") {
                    if let Some(id) = record_id(record) {
                        let del_url = format!("{BASE_URL}/dns/zones/{zone_id}/records/{id}");
                        let _ = http::delete(&del_url, &[("X-TCPanel-Token", &self.token)]);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Neodigit {
    fn get_root(&self, domain: &str, name: &str) -> Result<(String, String), Error> {
        let full = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len().saturating_sub(1) {
            let h = parts[i..].join(".");
            let url = format!("{BASE_URL}/dns/zones?name={h}");
            let resp = http::get(&url, &[("X-TCPanel-Token", &self.token)])
                .map_err(|e| Error::Provider(format!("neodigit zone lookup: {e}")))?;
            if resp.status >= 400 {
                continue;
            }
            let v: Value = serde_json::from_str(&resp.body)
                .map_err(|e| Error::Provider(format!("neodigit zone lookup: {e}")))?;
            let zones = v.as_array()
                .or_else(|| v.get("data").and_then(|d| d.as_array()))
                .or_else(|| v.get("zones").and_then(|z| z.as_array()));
            if let Some(zones) = zones {
                for zone in zones {
                    if zone.get("name").and_then(|n| n.as_str()) == Some(&h) {
                        if let Some(id) = record_id(zone) {
                            let sub = if i == 0 {
                                full.to_string()
                            } else {
                                let prefix = parts[..i].join(".");
                                if full.is_empty() {
                                    prefix
                                } else {
                                    format!("{prefix}.{full}")
                                }
                            };
                            return Ok((id, sub));
                        }
                    }
                }
            }
        }
        Err(Error::Provider("neodigit: could not resolve zone".into()))
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
