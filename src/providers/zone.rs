use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Zone {
    basic_auth: String,
}

impl DnsProvider for Zone {
    fn slug() -> &'static str {
        "zone"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ZONE_Username", "ZONE_Key"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("ZONE_Username")
            .ok_or_else(|| Error::Config("ZONE_Username required".into()))?
            .clone();
        let key = env.get("ZONE_Key")
            .ok_or_else(|| Error::Config("ZONE_Key required".into()))?
            .clone();
        let creds = format!("{username}:{key}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Zone { basic_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let body = serde_json::json!({
            "type": "TXT",
            "name": name,
            "destination": value,
            "ttl": 120,
        });
        let url = format!("https://api.zone.eu/v2/dns/{domain}/records");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("zone add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("zone add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let list_url = format!("https://api.zone.eu/v2/dns/{domain}/records");
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records = if let Some(arr) = v.as_array() {
            Some(arr)
        } else {
            v.get("data").and_then(|d| d.as_array())
        };
        if let Some(arr) = records {
            for r in arr {
                if r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && r.get("name").and_then(|n| n.as_str()) == Some(name)
                    && r.get("destination").and_then(|d| d.as_str()) == Some(value)
                {
                    if let Some(id) = r.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://api.zone.eu/v2/dns/{domain}/records/{id}");
                        let _ = http::delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}
