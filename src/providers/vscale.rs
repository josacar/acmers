use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Vscale {
    api_key: String,
}

impl DnsProvider for Vscale {
    fn slug() -> &'static str {
        "vscale"
    }

    fn env_vars() -> &'static [&'static str] {
        &["VSCALE_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("VSCALE_API_KEY")
            .ok_or_else(|| Error::Config("VSCALE_API_KEY required".into()))?
            .clone();
        Ok(Box::new(Vscale { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("X-Token", &self.api_key)];
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "name": name,
            "content": value,
            "ttl": 120,
        })).unwrap();
        let url = format!("https://api.vscale.io/v1/domains/{domain}/records");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("Vscale add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Vscale add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("X-Token", &self.api_key)];
        let list_url = format!("https://api.vscale.io/v1/domains/{domain}/records");
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records: Option<&Vec<Value>> = if let Some(arr) = v.as_array() {
            Some(arr)
        } else {
            v.get("records").and_then(|r| r.as_array())
        };
        if let Some(arr) = records {
            for record in arr {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://api.vscale.io/v1/domains/{domain}/records/{id}");
                        let _ = http::delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}
