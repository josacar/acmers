use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Easydns {
    basic_auth: String,
}

impl DnsProvider for Easydns {
    fn slug() -> &'static str {
        "easydns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["EASYDNS_Token", "EASYDNS_Key"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("EASYDNS_Token")
            .ok_or_else(|| Error::Config("EASYDNS_Token required".into()))?.clone();
        let key = env.get("EASYDNS_Key")
            .ok_or_else(|| Error::Config("EASYDNS_Key required".into()))?.clone();
        let creds = format!("{token}:{key}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Easydns { basic_auth }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone, sub) = self.resolve_zone(name)?;
        let url = format!("https://rest.easydns.net/zones/records/add/{zone}/TXT");
        let body = serde_json::json!({"host": sub, "rdata": value});
        let headers = &[
            ("Authorization", self.basic_auth.as_str()),
            ("Accept", "application/json"),
        ];
        let resp = http::put(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("easydns add TXT: {e}")))?;
        if resp.body.contains("\"status\":201") || resp.body.contains("Record already exists") {
            return Ok(());
        }
        Err(Error::Provider(format!("easydns add TXT: {}", resp.body)))
    }

    fn remove_txt(&self, _domain: &str, name: &str, _value: &str) -> ProviderResult {
        let (zone, sub) = self.resolve_zone(name)?;
        let url = format!("https://rest.easydns.net/zones/records/all/{zone}/search/{sub}");
        let headers = &[
            ("Authorization", self.basic_auth.as_str()),
            ("Accept", "application/json"),
        ];
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("easydns search records: {e}")))?;
        if !resp.body.contains("\"status\":200") {
            return Ok(());
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("easydns search parse: {e}")))?;
        let count = v.get("count").and_then(|c| c.as_u64()).unwrap_or(0);
        if count == 0 {
            return Ok(());
        }
        let record_id = v.get("records")
            .and_then(|r| r.as_array())
            .and_then(|arr| arr.first())
            .and_then(|r| r.get("id"))
            .and_then(|i| i.as_str())
            .map(|s| s.to_string());
        if let Some(id) = record_id {
            let del_url = format!("https://rest.easydns.net/zones/records/{zone}/{id}");
            http::delete(&del_url, headers)
                .map_err(|e| Error::Provider(format!("easydns delete TXT: {e}")))?;
        }
        Ok(())
    }
}

impl Easydns {
    fn resolve_zone(&self, fulldomain: &str) -> Result<(String, String), Error> {
        let headers = &[
            ("Authorization", self.basic_auth.as_str()),
            ("Accept", "application/json"),
        ];
        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                break;
            }
            let url = format!("https://rest.easydns.net/zones/records/all/{h}");
            let resp = http::get(&url, headers)
                .map_err(|e| Error::Provider(format!("easydns resolve zone: {e}")))?;
            if resp.body.contains("\"status\":200") {
                let sub = parts[..i].join(".");
                return Ok((h, sub));
            }
        }
        Err(Error::Provider(format!("zone not found for {fulldomain}")))
    }
}
