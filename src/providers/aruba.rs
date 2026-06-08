use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.aruba.it/v1";

pub struct Aruba {
    auth: String,
}

impl DnsProvider for Aruba {
    fn slug() -> &'static str {
        "aruba"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ARUBA_USERNAME", "ARUBA_PASSWORD"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("ARUBA_USERNAME")
            .ok_or_else(|| Error::Config("ARUBA_USERNAME required".into()))?
            .clone();
        let password = env.get("ARUBA_PASSWORD")
            .ok_or_else(|| Error::Config("ARUBA_PASSWORD required".into()))?
            .clone();
        let creds = format!("{username}:{password}");
        let auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Aruba { auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let _ = self.resolve_domain(domain)?;
        let body = serde_json::json!({
            "type": "TXT",
            "name": name,
            "content": value,
            "ttl": 120,
        });
        let url = format!("{BASE_URL}/domains/{domain}/dns-records");
        let headers: &[(&str, &str)] = &[("Authorization", &self.auth)];
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("aruba add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("aruba add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let _ = match self.resolve_domain(domain) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };
        let list_url = format!("{BASE_URL}/domains/{domain}/dns-records");
        let headers: &[(&str, &str)] = &[("Authorization", &self.auth)];
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records = v.as_array()
            .or_else(|| v.get("data").and_then(|d| d.as_array()))
            .or_else(|| v.get("records").and_then(|r| r.as_array()))
            .or_else(|| v.get("dns_records").and_then(|r| r.as_array()));
        if let Some(records) = records {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("{BASE_URL}/domains/{domain}/dns-records/{id}");
                        let _ = http::delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Aruba {
    fn resolve_domain(&self, domain: &str) -> Result<String, Error> {
        let url = format!("{BASE_URL}/domains");
        let headers: &[(&str, &str)] = &[("Authorization", &self.auth)];
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("aruba list domains: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("aruba list domains: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("aruba parse domains: {e}")))?;
        if let Some(arr) = v.as_array() {
            for d in arr {
                if let Some(nm) = d.get("domain").or_else(|| d.get("name")).and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        return Ok(nm.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("aruba: domain not found for {domain}")))
    }
}
