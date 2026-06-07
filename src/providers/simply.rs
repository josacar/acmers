use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Simply {
    basic_auth: String,
}

impl DnsProvider for Simply {
    fn slug() -> &'static str {
        "simply"
    }

    fn env_vars() -> &'static [&'static str] {
        &["SIMPLY_ApiLogin", "SIMPLY_ApiKey"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let login = env.get("SIMPLY_ApiLogin")
            .ok_or_else(|| Error::Config("SIMPLY_ApiLogin required".into()))?
            .clone();
        let api_key = env.get("SIMPLY_ApiKey")
            .ok_or_else(|| Error::Config("SIMPLY_ApiKey required".into()))?
            .clone();
        let creds = format!("{login}:{api_key}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Simply { basic_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "name": name,
            "data": value,
            "ttl": 120,
        })).unwrap();
        let url = format!("https://api.simply.com/2/my/products/{domain}/dns/records");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("Simply add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Simply add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let list_url = format!("https://api.simply.com/2/my/products/{domain}/dns/records");
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
                    && record.get("data").and_then(|d| d.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://api.simply.com/2/my/products/{domain}/dns/records/{id}");
                        let _ = http::delete(&del_url, headers);
                    }
                }
            }
        }
        Ok(())
    }
}
