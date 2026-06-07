use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Ionos {
    api_key: String,
    endpoint: String,
}

impl DnsProvider for Ionos {
    fn slug() -> &'static str {
        "ionos"
    }

    fn env_vars() -> &'static [&'static str] {
        &["IONOS_PREFIX", "IONOS_SECRET", "IONOS_ENDPOINT"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let prefix = env.get("IONOS_PREFIX")
            .ok_or_else(|| Error::Config("IONOS_PREFIX required".into()))?.clone();
        let secret = env.get("IONOS_SECRET")
            .ok_or_else(|| Error::Config("IONOS_SECRET required".into()))?.clone();
        let endpoint = env.get("IONOS_ENDPOINT")
            .cloned()
            .unwrap_or_else(|| "https://api.hosting.ionos.com/dns/v1".to_string());
        let api_key = format!("{prefix}.{secret}");
        Ok(Box::new(Ionos { api_key, endpoint }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let url = format!("{}/zones/{zone_id}/records", self.endpoint);
        let body = serde_json::json!({
            "name": name,
            "type": "TXT",
            "content": value,
            "ttl": 120,
        });
        http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("X-API-Key", &self.api_key)])
            .map_err(|e| Error::Provider(format!("ionos add TXT: {e}")))?;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let record_id = self.find_record_id(&zone_id, name, value)?;
        if let Some(id) = record_id {
            let url = format!("{}/zones/{zone_id}/records/{id}", self.endpoint);
            http::delete(&url, &[("X-API-Key", &self.api_key)])
                .map_err(|e| Error::Provider(format!("ionos delete TXT: {e}")))?;
        }
        Ok(())
    }
}

impl Ionos {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let url = format!("{}/zones", self.endpoint);
        let resp = http::get(&url, &[("X-API-Key", &self.api_key)])
            .map_err(|e| Error::Provider(format!("ionos list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("ionos zones: {e}")))?;
        if let Some(arr) = v.as_array() {
            for z in arr {
                if let Some(nm) = z.get("name").and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        if let Some(id) = z.get("id").and_then(|i| i.as_str()) {
                            return Ok(id.to_string());
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }

    fn find_record_id(&self, zone_id: &str, name: &str, value: &str) -> Result<Option<String>, Error> {
        let url = format!("{}/zones/{zone_id}/records", self.endpoint);
        let resp = http::get(&url, &[("X-API-Key", &self.api_key)])
            .map_err(|e| Error::Provider(format!("ionos list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("ionos records: {e}")))?;
        if let Some(arr) = v.as_array() {
            for r in arr {
                if r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && r.get("name").and_then(|n| n.as_str()) == Some(name)
                    && r.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = r.get("id").and_then(|i| i.as_str()) {
                        return Ok(Some(id.to_string()));
                    }
                }
            }
        }
        Ok(None)
    }
}
