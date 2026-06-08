use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Nsone {
    api_key: String,
}

impl DnsProvider for Nsone {
    fn slug() -> &'static str {
        "nsone"
    }

    fn env_vars() -> &'static [&'static str] {
        &["NS1_Key"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("NS1_Key")
            .ok_or_else(|| Error::Config("NS1_Key required".into()))?
            .clone();
        Ok(Box::new(Nsone { api_key }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("X-NSONE-Key", &self.api_key), ("Accept", "application/json")];
        let zone = self.resolve_zone(name)?;
        let resp = http::get(&format!("https://api.nsone.net/v1/zones/{zone}"), headers)
            .map_err(|e| Error::Provider(format!("nsone get zone: {e}")))?;
        let zone_data: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("nsone parse zone: {e}")))?;
        let count = zone_data.get("records").and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter(|r| {
                r.get("domain").and_then(|d| d.as_str()) == Some(name)
                    && r.get("type").and_then(|t| t.as_str()) == Some("TXT")
            }).count())
            .unwrap_or(0);
        let url = format!("https://api.nsone.net/v1/zones/{zone}/{name}/TXT");
        if count == 0 {
            let body = serde_json::json!({
                "answers": [{"answer": [value]}],
                "type": "TXT",
                "domain": name,
                "zone": zone,
                "ttl": 0,
            });
            http::put(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
                .map_err(|e| Error::Provider(format!("nsone add TXT: {e}")))?;
        } else {
            let prev_answers: Vec<Value> = zone_data.get("records").and_then(|r| r.as_array())
                .map(|arr| arr.iter().filter(|r| {
                    r.get("domain").and_then(|d| d.as_str()) == Some(name)
                        && r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                }).filter_map(|r| {
                    r.get("short_answers").and_then(|a| a.as_array()).cloned()
                }).flatten().map(|a| {
                    serde_json::json!({"answer": [a]})
                }).collect())
                .unwrap_or_default();
            let mut answers = vec![serde_json::json!({"answer": [value]})];
            answers.extend(prev_answers);
            let body = serde_json::json!({
                "answers": answers,
                "type": "TXT",
                "domain": name,
                "zone": zone,
                "ttl": 0,
            });
            http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
                .map_err(|e| Error::Provider(format!("nsone update TXT: {e}")))?;
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, _value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("X-NSONE-Key", &self.api_key)];
        let zone = match self.resolve_zone(name) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let url = format!("https://api.nsone.net/v1/zones/{zone}/{name}/TXT");
        let _ = http::delete(&url, headers);
        Ok(())
    }
}

impl Nsone {
    fn resolve_zone(&self, fulldomain: &str) -> Result<String, Error> {
        let headers: &[(&str, &str)] = &[("X-NSONE-Key", &self.api_key), ("Accept", "application/json")];
        let resp = http::get("https://api.nsone.net/v1/zones", headers)
            .map_err(|e| Error::Provider(format!("nsone list zones: {e}")))?;
        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 2..=parts.len() {
            let candidate = parts[i..].join(".");
            if candidate.is_empty() {
                break;
            }
            if resp.body.contains(&format!("\"zone\":\"{candidate}\"")) {
                return Ok(candidate);
            }
        }
        Err(Error::Provider(format!("zone not found for {fulldomain}")))
    }
}
