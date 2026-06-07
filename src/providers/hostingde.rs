use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Hostingde {
    api_key: String,
    endpoint: String,
}

impl DnsProvider for Hostingde {
    fn slug() -> &'static str {
        "hostingde"
    }

    fn env_vars() -> &'static [&'static str] {
        &["HOSTINGDE_APIKEY", "HOSTINGDE_ENDPOINT"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("HOSTINGDE_APIKEY")
            .ok_or_else(|| Error::Config("HOSTINGDE_APIKEY required".into()))?
            .clone();
        let endpoint = env.get("HOSTINGDE_ENDPOINT")
            .cloned()
            .unwrap_or_else(|| "https://secure.hosting.de/api/dns/v1/json".to_string());
        Ok(Box::new(Hostingde { api_key, endpoint }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = self.resolve_zone(domain)?;
        let record_name = name.strip_suffix(&format!(".{zone}")).unwrap_or(name);

        let body = serde_json::json!({
            "authToken": self.api_key,
            "method": "zoneUpdate",
            "params": {
                "zoneConfig": {"name": zone},
                "recordsToAdd": [{
                    "name": record_name,
                    "type": "TXT",
                    "content": value,
                    "ttl": 120,
                }]
            }
        });
        let resp = http::post(&self.endpoint, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("hostingde add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("hostingde add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("hostingde add TXT response: {e}")))?;
        if v.get("status").and_then(|s| s.as_str()) == Some("error") {
            if let Some(msg) = v.get("errors").and_then(|e| e.as_array()).and_then(|a| a.first()).and_then(|e| e.get("message").or_else(|| e.get("text"))).and_then(|m| m.as_str()) {
                return Err(Error::Provider(format!("hostingde add TXT: {msg}")));
            }
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = match self.resolve_zone(domain) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let record_name = name.strip_suffix(&format!(".{zone}")).unwrap_or(name);

        let record = match self.find_record(&zone, record_name, value) {
            Ok(Some(r)) => r,
            _ => return Ok(()),
        };

        let body = serde_json::json!({
            "authToken": self.api_key,
            "method": "zoneUpdate",
            "params": {
                "zoneConfig": {"name": zone},
                "recordsToDelete": [record]
            }
        });
        let _ = http::post(&self.endpoint, &serde_json::to_vec(&body).unwrap(), "application/json", &[]);
        Ok(())
    }
}

impl Hostingde {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len().saturating_sub(1) {
            let candidate: String = parts[i..].join(".");
            let body = serde_json::json!({
                "authToken": self.api_key,
                "method": "zoneFind",
                "params": {
                    "filter": {"field": "zoneName", "value": candidate}
                }
            });
            let resp = match http::post(&self.endpoint, &serde_json::to_vec(&body).unwrap(), "application/json", &[]) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let v: Value = match serde_json::from_str(&resp.body) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if let Some(data) = v.get("response").and_then(|r| r.get("data")).and_then(|d| d.as_array()) {
                if let Some(first) = data.first() {
                    if let Some(name) = first.get("zoneName").or_else(|| first.get("name")).and_then(|n| n.as_str()) {
                        return Ok(name.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("hostingde: zone not found for {domain}")))
    }

    fn find_record(&self, zone: &str, name: &str, value: &str) -> Result<Option<Value>, Error> {
        let body = serde_json::json!({
            "authToken": self.api_key,
            "method": "zoneInfo",
            "params": {
                "zoneConfig": {"name": zone}
            }
        });
        let resp = http::post(&self.endpoint, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("hostingde zone info: {e}")))?;

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("hostingde zone info: {e}")))?;

        if let Some(records) = v.get("response").and_then(|r| r.get("records")).and_then(|r| r.as_array()) {
            for record in records {
                if record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    return Ok(Some(record.clone()));
                }
            }
        }
        Ok(None)
    }
}
