use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Active24 {
    token: String,
}

impl DnsProvider for Active24 {
    fn slug() -> &'static str {
        "active24"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ACTIVE24_Token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("ACTIVE24_Token")
            .ok_or_else(|| Error::Config("ACTIVE24_Token required".into()))?
            .clone();
        Ok(Box::new(Active24 { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "name": name,
            "content": value,
            "ttl": 120,
        })).unwrap();
        let url = format!("https://api.active24.com/v2/dns/domains/{domain}/records/v1");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("Active24 add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Active24 add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let list_url = format!("https://api.active24.com/v2/dns/domains/{domain}/records/v1");
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records: Option<&Vec<Value>> = v.as_array()
            .or_else(|| v.get("records").and_then(|r| r.as_array()));
        if let Some(arr) = records {
            for record in arr {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://api.active24.com/v2/dns/domains/{domain}/records/{id}");
                        let _ = http::delete(&del_url, headers);
                    }
                }
            }
        }
        Ok(())
    }
}
