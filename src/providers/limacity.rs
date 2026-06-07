use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.lima-city.de/v1";

pub struct Limacity {
    api_key: String,
}

impl DnsProvider for Limacity {
    fn slug() -> &'static str {
        "limacity"
    }

    fn env_vars() -> &'static [&'static str] {
        &["LIMACITY_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("LIMACITY_API_KEY")
            .ok_or_else(|| Error::Config("LIMACITY_API_KEY required".into()))?
            .clone();
        Ok(Box::new(Limacity { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.api_key);
        let rec_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);

        let body = serde_json::json!({
            "domain": domain,
            "type": "TXT",
            "name": rec_name,
            "value": value,
            "ttl": 120,
        });
        let url = format!("{BASE_URL}/dns/records");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("limacity add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("limacity add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("limacity response: {e}")))?;
        if let Some(err) = v.get("error").and_then(|e| e.as_str())
            .or_else(|| v.get("message").and_then(|m| m.as_str())) {
            return Err(Error::Provider(format!("limacity add TXT: {err}")));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.api_key);
        let rec_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);

        let list_url = format!("{BASE_URL}/dns/records?domain={domain}");
        let resp = match http::get(&list_url, &[("Authorization", &auth)]) {
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
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(rec_name)
                {
                    if let Some(id) = record_id(record) {
                        let del_url = format!("{BASE_URL}/dns/records/{id}");
                        let _ = http::delete(&del_url, &[("Authorization", &auth)]);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
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
