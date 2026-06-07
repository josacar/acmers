use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Hetznercloud {
    token: String,
}

impl DnsProvider for Hetznercloud {
    fn slug() -> &'static str {
        "hetznercloud"
    }

    fn env_vars() -> &'static [&'static str] {
        &["HETZNERCLOUD_Token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("HETZNERCLOUD_Token")
            .ok_or_else(|| Error::Config("HETZNERCLOUD_Token required".into()))?
            .clone();
        Ok(Box::new(Hetznercloud { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Auth-API-Token", &self.token)];
        let zone_id = self.resolve_zone(domain, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "zone_id": zone_id,
            "type": "TXT",
            "name": name,
            "value": value,
            "ttl": 120,
        })).unwrap();
        let resp = http::post("https://dns.hetzner.com/api/v1/records", &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("Hetzner add TXT: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Hetzner response: {e}")))?;
        if let Some(err) = v.get("error").and_then(|e| e.get("message").and_then(|m| m.as_str())) {
            return Err(Error::Provider(format!("Hetzner add TXT: {err}")));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Auth-API-Token", &self.token)];
        let zone_id = match self.resolve_zone(domain, headers) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let list_url = format!("https://dns.hetzner.com/api/v1/records?zone_id={zone_id}");
        let resp = match http::get(&list_url, headers) {
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
                    if let Some(id) = record.get("id").and_then(|i| i.as_str()) {
                        let del_url = format!("https://dns.hetzner.com/api/v1/records/{id}");
                        let _ = http::delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Hetznercloud {
    fn resolve_zone(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let resp = http::get("https://dns.hetzner.com/api/v1/zones", headers)
            .map_err(|e| Error::Provider(format!("Hetzner list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Hetzner zones: {e}")))?;
        if let Some(zones) = v.get("zones").and_then(|z| z.as_array()) {
            for zone in zones {
                if let Some(name) = zone.get("name").and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        if let Some(id) = zone.get("id").and_then(|i| i.as_str()) {
                            return Ok(id.to_string());
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
