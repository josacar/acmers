use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Bunny {
    api_key: String,
}

impl DnsProvider for Bunny {
    fn slug() -> &'static str {
        "bunny"
    }

    fn env_vars() -> &'static [&'static str] {
        &["BUNNY_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("BUNNY_API_KEY")
            .ok_or_else(|| Error::Config("BUNNY_API_KEY required".into()))?.clone();
        Ok(Box::new(Bunny { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let url = format!("https://api.bunny.net/dnszone/{zone_id}/records");
        let body = serde_json::json!({
            "Id": 0,
            "Type": 2,
            "Name": name,
            "Value": value,
            "TtlSeconds": 120,
        });
        http::put(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("AccessKey", &self.api_key)])
            .map_err(|e| Error::Provider(format!("bunny add TXT: {e}")))?;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let record_id = self.find_record_id(&zone_id, name, value)?;
        if let Some(id) = record_id {
            let url = format!("https://api.bunny.net/dnszone/{zone_id}/records/{id}");
            http::delete(&url, &[("AccessKey", &self.api_key)])
                .map_err(|e| Error::Provider(format!("bunny delete TXT: {e}")))?;
        }
        Ok(())
    }
}

impl Bunny {
    fn resolve_zone(&self, domain: &str) -> Result<i64, Error> {
        let resp = http::get("https://api.bunny.net/dnszone", &[("AccessKey", &self.api_key)])
            .map_err(|e| Error::Provider(format!("bunny list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("bunny zones: {e}")))?;
        if let Some(items) = v.get("Items").and_then(|i| i.as_array()) {
            for item in items {
                if let Some(nm) = item.get("Domain").and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        if let Some(id) = item.get("Id").and_then(|i| i.as_i64()) {
                            return Ok(id);
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }

    fn find_record_id(&self, zone_id: &i64, name: &str, value: &str) -> Result<Option<i64>, Error> {
        let url = format!("https://api.bunny.net/dnszone/{zone_id}");
        let resp = http::get(&url, &[("AccessKey", &self.api_key)])
            .map_err(|e| Error::Provider(format!("bunny get zone: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("bunny zone: {e}")))?;
        if let Some(records) = v.get("Records").and_then(|r| r.as_array()) {
            for r in records {
                if r.get("Name").and_then(|n| n.as_str()) == Some(name)
                    && r.get("Value").and_then(|n| n.as_str()) == Some(value)
                    && r.get("Type").and_then(|t| t.as_i64()) == Some(2)
                {
                    if let Some(id) = r.get("Id").and_then(|i| i.as_i64()) {
                        return Ok(Some(id));
                    }
                }
            }
        }
        Ok(None)
    }
}
