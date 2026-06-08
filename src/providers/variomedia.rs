use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Variomedia {
    token: String,
}

impl DnsProvider for Variomedia {
    fn slug() -> &'static str {
        "variomedia"
    }

    fn env_vars() -> &'static [&'static str] {
        &["VARIOMEDIA_API_TOKEN"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("VARIOMEDIA_API_TOKEN")
            .ok_or_else(|| Error::Config("VARIOMEDIA_API_TOKEN required".into()))?
            .clone();
        Ok(Box::new(Variomedia { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let root = self.resolve_domain(domain)?;
        let sub = Self::sub_domain(name, &root);
        let auth = format!("token {}", self.token);
        let headers: &[(&str, &str)] = &[
            ("Authorization", &auth),
            ("Accept", "application/vnd.variomedia.v1+json"),
        ];
        let body = serde_json::to_vec(&serde_json::json!({
            "data": {
                "type": "dns-record",
                "attributes": {
                    "record_type": "TXT",
                    "name": sub,
                    "domain": root,
                    "data": value,
                    "ttl": 300
                }
            }
        })).unwrap();
        let resp = http::post("https://api.variomedia.de/dns-records", &body, "application/vnd.api+json", headers)
            .map_err(|e| Error::Provider(format!("Variomedia add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Variomedia add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let root = match self.resolve_domain(domain) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let sub = Self::sub_domain(name, &root);
        let auth = format!("token {}", self.token);
        let headers: &[(&str, &str)] = &[
            ("Authorization", &auth),
            ("Accept", "application/vnd.variomedia.v1+json"),
        ];
        let list_url = format!("https://api.variomedia.de/dns-records?filter[domain]={}", root);
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
            for record in arr {
                let attrs = match record.get("attributes") {
                    Some(a) => a,
                    None => continue,
                };
                let rec_name = attrs.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let rec_data = attrs.get("data").and_then(|d| d.as_str()).unwrap_or("");
                if rec_name == sub && rec_data == value {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://api.variomedia.de/dns-records/{}", id);
                        let _ = http::delete(&del_url, headers);
                    }
                }
            }
        }
        Ok(())
    }
}

impl Variomedia {
    fn resolve_domain(&self, domain: &str) -> Result<String, Error> {
        let auth = format!("token {}", self.token);
        let headers: &[(&str, &str)] = &[
            ("Authorization", &auth),
            ("Accept", "application/vnd.variomedia.v1+json"),
        ];
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len() {
            let candidate = parts[i..].join(".");
            let url = format!("https://api.variomedia.de/domains/{}", candidate);
            if let Ok(resp) = http::get(&url, &headers) {
                if let Ok(v) = serde_json::from_str::<Value>(&resp.body) {
                    if let Some(data) = v.get("data") {
                        if let Some(id) = data.get("id").and_then(|i| i.as_str()) {
                            if id == candidate {
                                return Ok(candidate);
                            }
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("domain not found: {domain}")))
    }

    fn sub_domain<'a>(full: &'a str, root: &str) -> &'a str {
        if let Some(stripped) = full.strip_suffix(root) {
            stripped.trim_end_matches('.')
        } else {
            full
        }
    }
}
