use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Poweradmin {
    url: String,
    api_key: String,
    api_version: String,
}

impl DnsProvider for Poweradmin {
    fn slug() -> &'static str {
        "poweradmin"
    }

    fn env_vars() -> &'static [&'static str] {
        &["POWERADMIN_URL", "POWERADMIN_API_KEY", "POWERADMIN_API_VERSION"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let url = env.get("POWERADMIN_URL")
            .ok_or_else(|| Error::Config("POWERADMIN_URL required".into()))?
            .clone();
        let api_key = env.get("POWERADMIN_API_KEY")
            .ok_or_else(|| Error::Config("POWERADMIN_API_KEY required".into()))?
            .clone();
        let api_version = env.get("POWERADMIN_API_VERSION")
            .cloned()
            .unwrap_or_else(|| "2".to_string());
        Ok(Box::new(Poweradmin { url, api_key, api_version }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone_id(domain)?;

        let body = serde_json::json!({
            "name": name,
            "type": "TXT",
            "content": value,
            "ttl": 60,
        });
        let url = format!("{}/api/v{}/zones/{}/records", self.url, self.api_version, zone_id);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("X-API-Key", &self.api_key)])
            .map_err(|e| Error::Provider(format!("poweradmin add TXT: {e}")))?;
        check_response(&resp, "poweradmin add TXT")?;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = match self.resolve_zone_id(domain) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };

        let list_url = format!("{}/api/v{}/zones/{}/records", self.url, self.api_version, zone_id);
        let resp = match http::get(&list_url, &[("X-API-Key", &self.api_key)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records = v.get("data").and_then(|d| d.as_array());
        if let Some(records) = records {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record_id(record) {
                        let del_url = format!("{}/api/v{}/zones/{}/records/{}", self.url, self.api_version, zone_id, id);
                        let _ = http::delete(&del_url, &[("X-API-Key", &self.api_key)]);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Poweradmin {
    fn resolve_zone_id(&self, domain: &str) -> Result<String, Error> {
        let url = format!("{}/api/v{}/zones", self.url, self.api_version);
        let resp = http::get(&url, &[("X-API-Key", &self.api_key)])
            .map_err(|e| Error::Provider(format!("poweradmin zones: {e}")))?;
        check_response(&resp, "poweradmin zones")?;

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Provider(format!("poweradmin zones JSON: {e}")))?;

        let zones = v.get("data").and_then(|d| d.as_array())
            .or_else(|| v.as_array());
        let zones = zones.ok_or_else(|| Error::Provider("poweradmin zones: no data".into()))?;

        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len() {
            let candidate = parts[i..].join(".");
            for zone in zones {
                if zone.get("name").and_then(|n| n.as_str()) == Some(&candidate) {
                    if let Some(id) = zone.get("id").and_then(|id| {
                        if id.is_u64() { id.as_u64().map(|n| n.to_string()) }
                        else if id.is_i64() { id.as_i64().map(|n| n.to_string()) }
                        else if id.is_string() { id.as_str().map(|s| s.to_string()) }
                        else { None }
                    }) {
                        return Ok(id);
                    }
                }
            }
        }
        Err(Error::Provider(format!("poweradmin: zone not found for {domain}")))
    }
}

fn check_response(resp: &crate::http::Response, ctx: &str) -> Result<(), Error> {
    if resp.status >= 400 {
        return Err(Error::Provider(format!("{ctx}: HTTP {} {}", resp.status, resp.body)));
    }
    if let Ok(v) = serde_json::from_str::<Value>(&resp.body) {
        if v.get("success").and_then(|s| s.as_bool()) == Some(false) {
            let msg = v.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error");
            return Err(Error::Provider(format!("{ctx}: {msg}")));
        }
    }
    Ok(())
}

fn record_id(v: &Value) -> Option<String> {
    v.get("id").and_then(|i| {
        if i.is_string() { i.as_str().map(|s| s.to_string()) }
        else if i.is_u64() { i.as_u64().map(|n| n.to_string()) }
        else if i.is_i64() { i.as_i64().map(|n| n.to_string()) }
        else { None }
    })
}
