use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const API_BASE: &str = "https://dnsapi.misaka.io/dns";

pub struct Misaka {
    token_auth: String,
}

impl DnsProvider for Misaka {
    fn slug() -> &'static str {
        "misaka"
    }

    fn env_vars() -> &'static [&'static str] {
        &["Misaka_Key"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let key = env.get("Misaka_Key")
            .ok_or_else(|| Error::Config("Misaka_Key required".into()))?
            .clone();
        let token_auth = format!("Token {key}");
        Ok(Box::new(Misaka { token_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.token_auth)];
        let (zone, sub) = self.resolve_zone(domain, name, headers)?;
        let search_url = format!("{API_BASE}/zones/{zone}/recordsets?search={sub}");
        let resp = http::get(&search_url, headers)
            .map_err(|e| Error::Provider(format!("Misaka search records: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Misaka search records: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Misaka search: {e}")))?;
        let count = count_txt_records(&v, &sub);
        let txt_value = format!("\"{}\"", value);
        if count == 0 {
            let body = serde_json::to_vec(&serde_json::json!({
                "records": [{"value": txt_value}],
                "filters": [],
                "ttl": 1,
            })).unwrap();
            let url = format!("{API_BASE}/zones/{zone}/recordsets/{sub}/TXT");
            let resp = http::post(&url, &body, "application/json", headers)
                .map_err(|e| Error::Provider(format!("Misaka add TXT: {e}")))?;
            if resp.status >= 400 {
                return Err(Error::Provider(format!("Misaka add TXT: HTTP {} {}", resp.status, resp.body)));
            }
        } else {
            let body = serde_json::to_vec(&serde_json::json!({
                "records": [{"value": txt_value}],
                "ttl": 1,
            })).unwrap();
            let url = format!("{API_BASE}/zones/{zone}/recordsets/{sub}/TXT?append=true");
            let resp = http::put(&url, &body, "application/json", headers)
                .map_err(|e| Error::Provider(format!("Misaka update TXT: {e}")))?;
            if resp.status >= 400 {
                return Err(Error::Provider(format!("Misaka update TXT: HTTP {} {}", resp.status, resp.body)));
            }
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.token_auth)];
        let (zone, sub) = match self.resolve_zone(domain, name, headers) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let url = format!("{API_BASE}/zones/{zone}/recordsets/{sub}/TXT");
        let _ = http::delete(&url, headers);
        Ok(())
    }
}

impl Misaka {
    fn resolve_zone(&self, domain: &str, fulldomain: &str, headers: &[(&str, &str)]) -> Result<(String, String), Error> {
        let url = format!("{API_BASE}/zones?limit=1000");
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("Misaka list zones: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Misaka list zones: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Misaka zones: {e}")))?;
        let zones: Vec<&str> = v.get("results").and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|z| z.get("name").and_then(|n| n.as_str())).collect())
            .unwrap_or_default();
        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 1..parts.len() {
            let candidate = parts[i..].join(".");
            if zones.contains(&candidate.as_str()) {
                let sub = parts[..i].join(".");
                return Ok((candidate, sub));
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}

fn count_txt_records(v: &Value, sub: &str) -> usize {
    let mut count = 0;
    if let Some(results) = v.get("results").and_then(|r| r.as_array()) {
        for rs in results {
            let name = rs.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let rtype = rs.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if name == sub && rtype == "TXT" {
                count += 1;
            }
        }
    }
    count
}
