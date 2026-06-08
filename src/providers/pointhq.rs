use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Pointhq {
    basic_auth: String,
}

impl DnsProvider for Pointhq {
    fn slug() -> &'static str {
        "pointhq"
    }

    fn env_vars() -> &'static [&'static str] {
        &["PointHQ_Key", "PointHQ_Email"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let email = env.get("PointHQ_Email")
            .ok_or_else(|| Error::Config("PointHQ_Email required".into()))?
            .clone();
        let key = env.get("PointHQ_Key")
            .ok_or_else(|| Error::Config("PointHQ_Key required".into()))?
            .clone();
        let creds = format!("{email}:{key}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Pointhq { basic_auth }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[
            ("Authorization", &self.basic_auth),
            ("Accept", "application/json"),
        ];
        let (zone_name, sub_domain) = self.resolve_zone(name, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "zone_record": {
                "name": sub_domain,
                "record_type": "TXT",
                "data": value,
                "ttl": 3600,
            }
        })).unwrap();
        let url = format!("https://api.pointhq.com/zones/{zone_name}/records");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("PointHQ add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("PointHQ add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, _value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[
            ("Authorization", &self.basic_auth),
            ("Accept", "application/json"),
        ];
        let (zone_name, sub_domain) = match self.resolve_zone(name, headers) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let list_url = format!("https://api.pointhq.com/zones/{zone_name}/records?record_type=TXT&name={sub_domain}");
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(records) = v.as_array() {
            for record in records {
                if let Some(id) = record.get("id").and_then(|i| {
                    if let Some(n) = i.as_i64() { Some(n) } else { i.as_str().and_then(|s| s.parse().ok()) }
                }) {
                    let del_url = format!("https://api.pointhq.com/zones/{zone_name}/records/{id}");
                    let _ = http::delete(&del_url, headers);
                }
            }
        }
        Ok(())
    }
}

impl Pointhq {
    fn resolve_zone(&self, fulldomain: &str, headers: &[(&str, &str)]) -> Result<(String, String), Error> {
        let parts: Vec<&str> = fulldomain.split('.').collect();
        let mut p = 1;
        let mut i = 2;
        loop {
            if i > parts.len() {
                return Err(Error::Provider(format!("zone not found for {fulldomain}")));
            }
            let h = parts[i - 1..].join(".");
            let resp = http::get("https://api.pointhq.com/zones", headers)
                .map_err(|e| Error::Provider(format!("PointHQ list zones: {e}")))?;
            if resp.body.contains(&format!("\"name\":\"{h}\"")) {
                let sub_domain = parts[..p].join(".");
                return Ok((h, sub_domain));
            }
            p = i;
            i += 1;
        }
    }
}
