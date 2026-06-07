use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Dynv6 {
    token: String,
}

impl DnsProvider for Dynv6 {
    fn slug() -> &'static str {
        "dynv6"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DYNV6_TOKEN"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("DYNV6_TOKEN")
            .ok_or_else(|| Error::Config("DYNV6_TOKEN required".into()))?
            .clone();
        Ok(Box::new(Dynv6 { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let auth = format!("Bearer {}", self.token);
        let url = format!("https://dynv6.com/api/v2/zones/{zone_id}/records");
        let body = serde_json::json!({
            "name": name,
            "type": "TXT",
            "data": value,
        });
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("dynv6 add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("dynv6 add TXT: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let auth = format!("Bearer {}", self.token);
        let list_url = format!("https://dynv6.com/api/v2/zones/{zone_id}/records");
        let resp = http::get(&list_url, &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("dynv6 list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("dynv6 parse: {e}")))?;
        if let Some(records) = v.as_array() {
            for rec in records {
                let rec_type = rec.get("type").and_then(|t| t.as_str());
                let rec_name = rec.get("name").and_then(|n| n.as_str());
                let rec_data = rec.get("data").and_then(|d| d.as_str());
                if rec_type == Some("TXT") && rec_name == Some(name) && rec_data == Some(value) {
                    if let Some(id) = rec.get("id").and_then(|i| i.as_str()) {
                        let del_url = format!("https://dynv6.com/api/v2/zones/{zone_id}/records/{id}");
                        http::delete(&del_url, &[("Authorization", &auth)]).ok();
                    }
                }
            }
        }
        Ok(())
    }
}

impl Dynv6 {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let auth = format!("Bearer {}", self.token);
        let url = "https://dynv6.com/api/v2/zones";
        let resp = http::get(url, &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("dynv6 list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("dynv6 parse: {e}")))?;
        if let Some(zones) = v.as_array() {
            for z in zones {
                if let Some(name) = z.get("name").and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        if let Some(id) = z.get("id").and_then(|i| i.as_str()) {
                            return Ok(id.to_string());
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("dynv6 zone not found for {domain}")))
    }
}
