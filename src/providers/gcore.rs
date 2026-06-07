use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Gcore {
    api_key: String,
}

impl DnsProvider for Gcore {
    fn slug() -> &'static str {
        "gcore"
    }

    fn env_vars() -> &'static [&'static str] {
        &["GCORE_PermanentAPIKey"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("GCORE_PermanentAPIKey")
            .ok_or_else(|| Error::Config("GCORE_PermanentAPIKey required".into()))?
            .clone();
        Ok(Box::new(Gcore { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("APIKey {}", self.api_key);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let zone_id = self.resolve_zone(domain, headers)?;
        let quoted_value = format!("\"{value}\"");
        let body = serde_json::to_vec(&serde_json::json!({
            "name": name,
            "type": "TXT",
            "records": [{"content": [&quoted_value]}],
            "ttl": 120,
        })).unwrap();
        let url = format!("https://api.gcore.com/dns/v2/zones/{zone_id}/rrsets");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("G-Core add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("G-Core add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let auth = format!("APIKey {}", self.api_key);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let zone_id = match self.resolve_zone(domain, headers) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let del_url = format!("https://api.gcore.com/dns/v2/zones/{zone_id}/rrsets/{name}/TXT");
        let _ = http::delete(&del_url, headers);
        Ok(())
    }
}

impl Gcore {
    fn resolve_zone(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let resp = http::get("https://api.gcore.com/dns/v2/zones", headers)
            .map_err(|e| Error::Provider(format!("G-Core list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("G-Core zones: {e}")))?;
        if let Some(zones) = v.as_array() {
            for zone in zones {
                if zone.get("name").and_then(|n| n.as_str()) == Some(domain) {
                    if let Some(id) = zone.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        return Ok(id);
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
