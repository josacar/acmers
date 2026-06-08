use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Pointhq {
    basic_auth: String,
}

impl DnsProvider for Pointhq {
    fn slug() -> &'static str {
        "pointhq"
    }

    fn env_vars() -> &'static [&'static str] {
        &["POINTHQ_User", "POINTHQ_Token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let user = env.get("POINTHQ_User")
            .ok_or_else(|| Error::Config("POINTHQ_User required".into()))?
            .clone();
        let token = env.get("POINTHQ_Token")
            .ok_or_else(|| Error::Config("POINTHQ_Token required".into()))?
            .clone();
        let creds = format!("{user}:{token}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Pointhq { basic_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let zone_id = self.resolve_zone(domain, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "record": {
                "name": name,
                "record_type": "TXT",
                "data": value,
                "ttl": 120,
            }
        })).unwrap();
        let url = format!("https://pointhq.com/api/zones/{zone_id}/records");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("PointHQ add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("PointHQ add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let zone_id = match self.resolve_zone(domain, headers) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let list_url = format!("https://pointhq.com/api/zones/{zone_id}/records?record_type=TXT&name={name}");
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
                if record.get("id").is_some() {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n) } else { i.as_str().and_then(|s| s.parse().ok()) }
                    }) {
                        let del_url = format!("https://pointhq.com/api/zones/{zone_id}/records/{id}");
                        let _ = http::delete(&del_url, headers);
                    }
                }
            }
        }
        Ok(())
    }
}

impl Pointhq {
    fn resolve_zone(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let resp = http::get("https://pointhq.com/api/zones", headers)
            .map_err(|e| Error::Provider(format!("PointHQ list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("PointHQ zones: {e}")))?;
        if let Some(zones) = v.as_array() {
            for z in zones {
                if let Some(zone) = z.get("zone").and_then(|n| n.get("name").and_then(|s| s.as_str())) {
                    if domain == zone || domain.ends_with(&format!(".{zone}")) {
                        if let Some(id) = z.get("zone").and_then(|n| n.get("id").and_then(|i| {
                            if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                        })) {
                            return Ok(id);
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
