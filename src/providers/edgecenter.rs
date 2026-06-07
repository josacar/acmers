use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Edgecenter {
    api_key: String,
}

impl DnsProvider for Edgecenter {
    fn slug() -> &'static str {
        "edgecenter"
    }

    fn env_vars() -> &'static [&'static str] {
        &["EDGECENTER_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("EDGECENTER_API_KEY")
            .ok_or_else(|| Error::Config("EDGECENTER_API_KEY required".into()))?
            .clone();
        Ok(Box::new(Edgecenter { api_key }))
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
        let url = format!("https://api.edgecenter.ru/dns/v2/zones/{zone_id}/rrsets");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("EdgeCenter add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("EdgeCenter add TXT: HTTP {} {}", resp.status, resp.body)));
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
        let del_url = format!("https://api.edgecenter.ru/dns/v2/zones/{zone_id}/rrsets/{name}/TXT");
        let _ = http::delete(&del_url, headers);
        Ok(())
    }
}

impl Edgecenter {
    fn resolve_zone(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let resp = http::get("https://api.edgecenter.ru/dns/v2/zones", headers)
            .map_err(|e| Error::Provider(format!("EdgeCenter list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("EdgeCenter zones: {e}")))?;
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
