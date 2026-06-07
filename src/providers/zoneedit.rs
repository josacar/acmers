use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Zoneedit {
    basic_auth: String,
}

impl DnsProvider for Zoneedit {
    fn slug() -> &'static str {
        "zoneedit"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ZONEEDIT_User", "ZONEEDIT_Token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let user = env.get("ZONEEDIT_User")
            .ok_or_else(|| Error::Config("ZONEEDIT_User required".into()))?
            .clone();
        let token = env.get("ZONEEDIT_Token")
            .ok_or_else(|| Error::Config("ZONEEDIT_Token required".into()))?
            .clone();
        let creds = format!("{user}:{token}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Zoneedit { basic_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "name": name,
            "content": value,
            "ttl": 120,
        })).unwrap();
        let url = format!("https://api.cp.zoneedit.com/dns/domains/{domain}/records");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("zoneedit add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("zoneedit add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let list_url = format!("https://api.cp.zoneedit.com/dns/domains/{domain}/records");
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(records) = v.as_array() {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://api.cp.zoneedit.com/dns/domains/{domain}/records/{id}");
                        let _ = http::delete(&del_url, headers);
                    }
                }
            }
        }
        Ok(())
    }
}
