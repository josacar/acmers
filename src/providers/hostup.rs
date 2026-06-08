use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://cloud.hostup.se/api";

pub struct Hostup {
    api_key: String,
}

impl DnsProvider for Hostup {
    fn slug() -> &'static str { "hostup" }
    fn env_vars() -> &'static [&'static str] { &["HOSTUP_API_KEY"] }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("HOSTUP_API_KEY")
            .ok_or_else(|| Error::Config("HOSTUP_API_KEY required".into()))?
            .clone();
        Ok(Box::new(Hostup { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.api_key);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let (zone_id, zone_domain) = self.resolve_zone(domain, headers)?;
        let rec_name = self.record_name(name, &zone_domain);
        let body = serde_json::to_vec(&serde_json::json!({
            "name": rec_name,
            "type": "TXT",
            "value": value,
            "ttl": 60,
        })).unwrap();
        let url = format!("{BASE_URL}/dns/zones/{zone_id}/records");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("hostup add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("hostup add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.api_key);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let zone_id = match self.resolve_zone(domain, headers) {
            Ok((id, _)) => id,
            Err(_) => return Ok(()),
        };
        let list_url = format!("{BASE_URL}/dns/zones/{zone_id}/records");
        let resp = match http::get(&list_url, headers) {
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
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("value").and_then(|v| v.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("{BASE_URL}/dns/zones/{zone_id}/records/{id}");
                        let _ = http::delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Hostup {
    fn record_name<'a>(&self, name: &'a str, zone_domain: &str) -> String {
        let suffix = format!(".{zone_domain}");
        let rel = name.strip_suffix(&suffix).unwrap_or(name);
        if rel.is_empty() || rel == "." {
            "@".to_string()
        } else {
            rel.to_string()
        }
    }

    fn resolve_zone(&self, domain: &str, headers: &[(&str, &str)]) -> Result<(String, String), Error> {
        let resp = http::get(&format!("{BASE_URL}/dns/zones"), headers)
            .map_err(|e| Error::Provider(format!("hostup list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("hostup zones: {e}")))?;
        let zones = v.as_array()
            .or_else(|| v.get("data").and_then(|d| d.as_array()));
        let zones = zones.ok_or_else(|| Error::Provider("hostup zones: unexpected response".into()))?;

        let mut candidate = domain.to_string();
        loop {
            for zone in zones {
                if let Some(zd) = zone.get("domain").and_then(|d| d.as_str()) {
                    if candidate == zd || candidate.ends_with(&format!(".{zd}")) {
                        if let Some(id) = zone.get("domain_id").and_then(|i| {
                            if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                        }) {
                            return Ok((id, zd.to_string()));
                        }
                    }
                }
            }
            if let Some(pos) = candidate.find('.') {
                candidate = candidate[pos + 1..].to_string();
            } else {
                break;
            }
        }
        Err(Error::Provider(format!("hostup: zone not found for {domain}")))
    }
}
