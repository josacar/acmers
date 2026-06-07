use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Mijnhost {
    api_key: String,
}

impl DnsProvider for Mijnhost {
    fn slug() -> &'static str {
        "mijnhost"
    }

    fn env_vars() -> &'static [&'static str] {
        &["MIJNHOST_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("MIJNHOST_API_KEY")
            .ok_or_else(|| Error::Config("MIJNHOST_API_KEY required".into()))?
            .clone();
        Ok(Box::new(Mijnhost { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.api_key);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let domain_id = self.resolve_domain(domain, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "name": name,
            "value": value,
            "ttl": 120,
        })).unwrap();
        let url = format!("https://mijn.host/api/v2/domains/{domain_id}/dns");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("Mijnhost add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Mijnhost add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.api_key);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let domain_id = match self.resolve_domain(domain, headers) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };
        let list_url = format!("https://mijn.host/api/v2/domains/{domain_id}/dns");
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records: Option<&Vec<Value>> = v.get("data").and_then(|d| d.as_array());
        if let Some(arr) = records {
            for record in arr {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("value").and_then(|v| v.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://mijn.host/api/v2/domains/{domain_id}/dns/{id}");
                        let _ = http::delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Mijnhost {
    fn resolve_domain(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let resp = http::get("https://mijn.host/api/v2/domains", headers)
            .map_err(|e| Error::Provider(format!("Mijnhost list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Mijnhost domains: {e}")))?;
        if let Some(domains) = v.get("data").and_then(|d| d.as_array()) {
            for d in domains {
                if d.get("domain").and_then(|n| n.as_str()) == Some(domain) {
                    if let Some(id) = d.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        return Ok(id);
                    }
                }
            }
        }
        Err(Error::Provider(format!("domain not found for {domain}")))
    }
}
