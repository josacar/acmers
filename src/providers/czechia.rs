use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.czechia.com/v1";

pub struct Czechia {
    api_key: String,
}

impl DnsProvider for Czechia {
    fn slug() -> &'static str {
        "czechia"
    }

    fn env_vars() -> &'static [&'static str] {
        &["CZECHIA_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("CZECHIA_API_KEY")
            .ok_or_else(|| Error::Config("CZECHIA_API_KEY required".into()))?
            .clone();
        Ok(Box::new(Czechia { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let rec_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let zone_id = self.resolve_zone(domain)?;
        let body = serde_json::json!({
            "type": "TXT",
            "name": rec_name,
            "content": value,
            "ttl": 120,
        });
        let url = format!("{BASE_URL}/dns/zones/{zone_id}/records");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("X-API-Key", &self.api_key)])
            .map_err(|e| Error::Provider(format!("czechia add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("czechia add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let rec_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let zone_id = match self.resolve_zone(domain) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let list_url = format!("{BASE_URL}/dns/zones/{zone_id}/records");
        let resp = match http::get(&list_url, &[("X-API-Key", &self.api_key)]) {
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
                        let del_url = format!("{BASE_URL}/dns/zones/{zone_id}/records/{id}");
                        let _ = http::delete(&del_url, &[("X-API-Key", &self.api_key)]);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Czechia {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let resp = http::get(&format!("{BASE_URL}/dns/zones"), &[("X-API-Key", &self.api_key)])
            .or_else(|_| http::get(&format!("{BASE_URL}/domains"), &[("X-API-Key", &self.api_key)]))
            .map_err(|e| Error::Provider(format!("czechia list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("czechia zones: {e}")))?;
        let zones = v.as_array()
            .or_else(|| v.get("data").and_then(|d| d.as_array()))
            .or_else(|| v.get("zones").and_then(|z| z.as_array()))
            .or_else(|| v.get("domains").and_then(|d| d.as_array()));
        if let Some(zones) = zones {
            for z in zones {
                if let Some(name) = z.get("name").or_else(|| z.get("domain")).and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        if let Some(id) = zone_id(z) {
                            return Ok(id);
                        }
                    }
                }
            }
        }
        Ok(domain.to_string())
    }
}

fn zone_id(v: &Value) -> Option<String> {
    v.get("id").and_then(|i| {
        if i.is_string() { i.as_str().map(|s| s.to_string()) }
        else if i.is_u64() { i.as_u64().map(|n| n.to_string()) }
        else if i.is_i64() { i.as_i64().map(|n| n.to_string()) }
        else { None }
    })
}

fn record_id(v: &Value) -> Option<String> {
    v.get("id").and_then(|i| {
        if i.is_string() { i.as_str().map(|s| s.to_string()) }
        else if i.is_u64() { i.as_u64().map(|n| n.to_string()) }
        else if i.is_i64() { i.as_i64().map(|n| n.to_string()) }
        else { None }
    })
}
