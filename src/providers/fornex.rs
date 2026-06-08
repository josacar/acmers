use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://fornex.com/api";

pub struct Fornex {
    api_key: String,
}

impl DnsProvider for Fornex {
    fn slug() -> &'static str {
        "fornex"
    }

    fn env_vars() -> &'static [&'static str] {
        &["FORNEX_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("FORNEX_API_KEY")
            .ok_or_else(|| Error::Config("FORNEX_API_KEY required".into()))?
            .clone();
        Ok(Box::new(Fornex { api_key }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_domain = self.resolve_zone(name)?;
        let body = serde_json::json!({
            "host": name,
            "type": "TXT",
            "value": value,
            "ttl": null,
        });
        let url = format!("{BASE_URL}/dns/domain/{zone_domain}/entry_set/");
        let auth = format!("Api-Key {}", self.api_key);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("Authorization", &auth), ("Accept", "application/json")])
            .map_err(|e| Error::Provider(format!("fornex add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("fornex add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_domain = match self.resolve_zone(name) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };
        let auth = format!("Api-Key {}", self.api_key);
        let list_url = format!("{BASE_URL}/dns/domain/{zone_domain}/entry_set?type=TXT&q={name}");
        let resp = match http::get(&list_url, &[("Authorization", &auth), ("Accept", "application/json")]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if !resp.body.contains(value) {
            return Ok(());
        }
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records = v.as_array()
            .or_else(|| v.get("data").and_then(|d| d.as_array()))
            .or_else(|| v.get("results").and_then(|r| r.as_array()));
        if let Some(records) = records {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("value").and_then(|v| v.as_str()) == Some(value)
                {
                    if let Some(id) = record_id(record) {
                        let del_url = format!("{BASE_URL}/dns/domain/{zone_domain}/entry_set/{id}/");
                        let _ = http::delete(&del_url, &[("Authorization", &auth), ("Accept", "application/json")]);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Fornex {
    fn resolve_zone(&self, fulldomain: &str) -> Result<String, Error> {
        let auth = format!("Api-Key {}", self.api_key);
        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                break;
            }
            let url = format!("{BASE_URL}/dns/domain/?q={h}");
            let resp = match http::get(&url, &[("Authorization", &auth), ("Accept", "application/json")]) {
                Ok(r) => r,
                Err(_) => continue,
            };
            if resp.body.contains(&format!("\"name\":\"{h}\"")) {
                return Ok(h);
            }
        }
        Err(Error::Provider("fornex: unable to determine root domain".into()))
    }
}

fn record_id(v: &Value) -> Option<String> {
    v.get("id").and_then(|i| {
        if i.is_string() { i.as_str().map(|s| s.to_string()) }
        else if i.is_u64() { i.as_u64().map(|n| n.to_string()) }
        else if i.is_i64() { i.as_i64().map(|n| n.to_string()) }
        else { None }
    })
}
