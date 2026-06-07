use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Vercel {
    token: String,
}

impl DnsProvider for Vercel {
    fn slug() -> &'static str {
        "vercel"
    }

    fn env_vars() -> &'static [&'static str] {
        &["VERCEL_TOKEN"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("VERCEL_TOKEN")
            .ok_or_else(|| Error::Config("VERCEL_TOKEN required".into()))?
            .clone();
        Ok(Box::new(Vercel { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "name": name,
            "value": value,
            "ttl": 60,
        })).unwrap();
        let url = format!("https://api.vercel.com/v2/domains/{domain}/records");
        let resp = http::post(&url, &body, "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("Vercel add TXT: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Vercel response: {e}")))?;
        if let Some(err) = v.get("error").and_then(|e| e.get("message").and_then(|m| m.as_str())) {
            return Err(Error::Provider(format!("Vercel add TXT: {err}")));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let list_url = format!("https://api.vercel.com/v2/domains/{domain}/records");
        let resp = match http::get(&list_url, &[("Authorization", &auth)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(records) = v.get("records").and_then(|r| r.as_array()) {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("value").and_then(|v| v.as_str()) == Some(value)
                {
                    if let Some(uid) = record.get("uid").and_then(|u| u.as_str()) {
                        let del_url = format!("https://api.vercel.com/v2/domains/{domain}/records/{uid}");
                        let _ = http::delete(&del_url, &[("Authorization", &auth)]);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}
