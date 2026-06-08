use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.artfiles.de/v1";

pub struct Artfiles {
    key: String,
    secret: String,
}

impl DnsProvider for Artfiles {
    fn slug() -> &'static str {
        "artfiles"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ARTFILES_Key", "ARTFILES_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let key = env.get("ARTFILES_Key")
            .ok_or_else(|| Error::Config("ARTFILES_Key required".into()))?
            .clone();
        let secret = env.get("ARTFILES_Secret")
            .ok_or_else(|| Error::Config("ARTFILES_Secret required".into()))?
            .clone();
        Ok(Box::new(Artfiles { key, secret }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let rec_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let body = serde_json::json!({
            "type": "TXT",
            "name": rec_name,
            "content": value,
            "ttl": 120,
        });
        let url = format!("{BASE_URL}/dns/{domain}/records");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("X-API-Key", &self.key), ("X-API-Secret", &self.secret)])
            .map_err(|e| Error::Provider(format!("artfiles add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("artfiles add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let rec_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let list_url = format!("{BASE_URL}/dns/{domain}/records");
        let headers: &[(&str, &str)] = &[("X-API-Key", &self.key), ("X-API-Secret", &self.secret)];
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
            .or_else(|| v.get("records").and_then(|r| r.as_array()));
        if let Some(records) = records {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(rec_name)
                {
                    if let Some(id) = record_id(record) {
                        let del_url = format!("{BASE_URL}/dns/{domain}/records/{id}");
                        let _ = http::delete(&del_url, headers);
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
