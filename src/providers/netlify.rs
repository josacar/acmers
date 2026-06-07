use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Netlify {
    token: String,
}

impl DnsProvider for Netlify {
    fn slug() -> &'static str {
        "netlify"
    }

    fn env_vars() -> &'static [&'static str] {
        &["NETLIFY_ACCESS_TOKEN"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("NETLIFY_ACCESS_TOKEN")
            .ok_or_else(|| Error::Config("NETLIFY_ACCESS_TOKEN required".into()))?.clone();
        Ok(Box::new(Netlify { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let url = format!("https://api.netlify.com/api/v1/dns_zones/{zone_id}/dns_records");
        let body = serde_json::json!({
            "type": "TXT",
            "hostname": name,
            "value": value,
            "ttl": 120,
        });
        let auth = format!("Bearer {}", self.token);
        http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("netlify add TXT: {e}")))?;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let record_id = self.find_record_id(&zone_id, name)?;
        if let Some(id) = record_id {
            let url = format!("https://api.netlify.com/api/v1/dns_zones/{zone_id}/dns_records/{id}");
            let auth = format!("Bearer {}", self.token);
            http::delete(&url, &[("Authorization", &auth)])
                .map_err(|e| Error::Provider(format!("netlify delete TXT: {e}")))?;
        }
        Ok(())
    }
}

impl Netlify {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let auth = format!("Bearer {}", self.token);
        let resp = http::get("https://api.netlify.com/api/v1/dns_zones", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("netlify list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("netlify zones: {e}")))?;
        if let Some(arr) = v.as_array() {
            for z in arr {
                if let Some(nm) = z.get("name").and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        if let Some(id) = z.get("id").and_then(|i| i.as_str()) {
                            return Ok(id.to_string());
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }

    fn find_record_id(&self, zone_id: &str, name: &str) -> Result<Option<String>, Error> {
        let auth = format!("Bearer {}", self.token);
        let url = format!("https://api.netlify.com/api/v1/dns_zones/{zone_id}/dns_records");
        let resp = http::get(&url, &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("netlify list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("netlify records: {e}")))?;
        if let Some(arr) = v.as_array() {
            for r in arr {
                if r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && r.get("hostname").and_then(|h| h.as_str()) == Some(name)
                {
                    if let Some(id) = r.get("id").and_then(|i| i.as_str()) {
                        return Ok(Some(id.to_string()));
                    }
                }
            }
        }
        Ok(None)
    }
}
