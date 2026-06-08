use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Ipv64 {
    api_key: String,
}

impl DnsProvider for Ipv64 {
    fn slug() -> &'static str {
        "ipv64"
    }

    fn env_vars() -> &'static [&'static str] {
        &["IPV64_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("IPV64_API_KEY")
            .ok_or_else(|| Error::Config("IPV64_API_KEY required".into()))?.clone();
        Ok(Box::new(Ipv64 { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.api_key);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let url = format!("https://ipv64.net/api?domain={}&add={}&type=TXT&content={}&ttl=120", domain, name, value);
        let resp = http::post(&url, &[], "application/x-www-form-urlencoded", headers)
            .map_err(|e| Error::Provider(format!("ipv64 add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("ipv64 add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.api_key);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let url = format!("https://ipv64.net/api?domain={}", domain);
        let resp = match http::get(&url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if resp.status >= 400 {
            return Ok(());
        }
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(records) = v.get("records").and_then(|r| r.as_array()) {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| i.as_i64()) {
                        let del_url = format!("https://ipv64.net/api?domain={}&del={}", domain, id);
                        let _ = http::post(&del_url, &[], "application/x-www-form-urlencoded", headers);
                    }
                    return Ok(());
                }
            }
        }
        Ok(())
    }
}
