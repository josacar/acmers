use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const DEFAULT_ENDPOINT: &str = "https://portal.nexcess.net";
const API_VERSION: &str = "0";
const USER_AGENT: &str = "NW-ACME-CLIENT";

pub struct Nw {
    token: String,
    endpoint: String,
}

impl DnsProvider for Nw {
    fn slug() -> &'static str {
        "nw"
    }

    fn env_vars() -> &'static [&'static str] {
        &["NW_API_TOKEN", "NW_API_ENDPOINT"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("NW_API_TOKEN")
            .ok_or_else(|| Error::Config("NW_API_TOKEN required".into()))?
            .clone();
        let endpoint = env.get("NW_API_ENDPOINT")
            .map(|s| s.trim_end_matches('/').to_string())
            .unwrap_or_else(|| DEFAULT_ENDPOINT.to_string());
        Ok(Box::new(Nw { token, endpoint }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let zone_id = self.resolve_zone(domain, &auth)?;
        let body = serde_json::json!({
            "zone_id": zone_id,
            "type": "TXT",
            "host": name,
            "target": value,
            "ttl": "300",
        });
        let url = format!("{}/dns-record", self.endpoint);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[
                ("Authorization", &auth),
                ("Api-Version", API_VERSION),
                ("User-Agent", USER_AGENT),
            ])
            .map_err(|e| Error::Provider(format!("nw add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("nw add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let zone_id = match self.resolve_zone(domain, &auth) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let list_url = format!("{}/dns-record?zone_id={}", self.endpoint, zone_id);
        let resp = match http::get(&list_url, &[
            ("Authorization", &auth),
            ("Api-Version", API_VERSION),
            ("User-Agent", USER_AGENT),
        ]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records = v.as_array()
            .or_else(|| v.get("data").and_then(|d| d.as_array()));
        if let Some(records) = records {
            for record in records {
                if record.get("host").and_then(|n| n.as_str()) == Some(name)
                    && record.get("target").and_then(|t| t.as_str()) == Some(value)
                {
                    if let Some(id) = record_id(record) {
                        let del_url = format!("{}/dns-record/{}", self.endpoint, id);
                        let _ = http::delete(&del_url, &[
                            ("Authorization", &auth),
                            ("Api-Version", API_VERSION),
                            ("User-Agent", USER_AGENT),
                        ]);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Nw {
    fn resolve_zone(&self, domain: &str, auth: &str) -> Result<String, Error> {
        let resp = http::get(&format!("{}/dns-zone", self.endpoint), &[
            ("Authorization", auth),
            ("Api-Version", API_VERSION),
            ("User-Agent", USER_AGENT),
        ])
            .map_err(|e| Error::Provider(format!("nw list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("nw zones: {e}")))?;
        let zones = v.as_array()
            .or_else(|| v.get("data").and_then(|d| d.as_array()));
        if let Some(zones) = zones {
            for z in zones {
                if let Some(name) = z.get("domain").and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        if let Some(id) = zone_id(z) {
                            return Ok(id);
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("nw: zone not found for {domain}")))
    }
}

fn zone_id(v: &Value) -> Option<String> {
    v.get("zone_id").and_then(|i| {
        if i.is_string() { i.as_str().map(|s| s.to_string()) }
        else if i.is_u64() { i.as_u64().map(|n| n.to_string()) }
        else if i.is_i64() { i.as_i64().map(|n| n.to_string()) }
        else { None }
    })
}

fn record_id(v: &Value) -> Option<String> {
    v.get("record_id").and_then(|i| {
        if i.is_string() { i.as_str().map(|s| s.to_string()) }
        else if i.is_u64() { i.as_u64().map(|n| n.to_string()) }
        else if i.is_i64() { i.as_i64().map(|n| n.to_string()) }
        else { None }
    })
}
