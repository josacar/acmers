use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Vultr {
    api_key: String,
}

impl DnsProvider for Vultr {
    fn slug() -> &'static str {
        "vultr"
    }

    fn env_vars() -> &'static [&'static str] {
        &["VULTR_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("VULTR_API_KEY")
            .ok_or_else(|| Error::Config("VULTR_API_KEY required".into()))?
            .clone();
        Ok(Box::new(Vultr { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.api_key);
        let url = format!("https://api.vultr.com/v2/domains/{domain}/records");
        let body = serde_json::json!({
            "type": "TXT",
            "name": name,
            "data": value,
            "ttl": 120,
        });
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("vultr add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("vultr add TXT: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.api_key);
        let list_url = format!("https://api.vultr.com/v2/domains/{domain}/records");
        let resp = http::get(&list_url, &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("vultr list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("vultr parse: {e}")))?;
        if let Some(records) = v.get("records").and_then(|r| r.as_array()) {
            for rec in records {
                let rec_type = rec.get("type").and_then(|t| t.as_str());
                let rec_name = rec.get("name").and_then(|n| n.as_str());
                let rec_data = rec.get("data").and_then(|d| d.as_str());
                if rec_type == Some("TXT") && rec_name == Some(name) && rec_data == Some(value) {
                    if let Some(id) = rec.get("id").and_then(|i| i.as_str()) {
                        let del_url = format!("https://api.vultr.com/v2/domains/{domain}/records/{id}");
                        http::delete(&del_url, &[("Authorization", &auth)]).ok();
                    }
                }
            }
        }
        Ok(())
    }
}
