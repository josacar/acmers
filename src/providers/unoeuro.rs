use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Unoeuro {
    user: String,
    key: String,
}

impl DnsProvider for Unoeuro {
    fn slug() -> &'static str {
        "unoeuro"
    }

    fn env_vars() -> &'static [&'static str] {
        &["UNOEURO_User", "UNOEURO_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let user = env.get("UNOEURO_User")
            .or_else(|| env.get("UNO_User"))
            .ok_or_else(|| Error::Config("UNOEURO_User (or UNO_User) required".into()))?
            .clone();
        let key = env.get("UNOEURO_Password")
            .or_else(|| env.get("UNO_Key"))
            .ok_or_else(|| Error::Config("UNOEURO_Password (or UNO_Key) required".into()))?
            .clone();
        Ok(Box::new(Unoeuro { user, key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = self.get_root(domain)?;
        let url = format!("https://api.simply.com/1/{}/{}/my/products/{zone}/dns/records", self.user, self.key);
        let body = serde_json::to_vec(&serde_json::json!({
            "name": name,
            "type": "TXT",
            "data": value,
            "ttl": 120,
            "priority": 0,
        })).unwrap();
        let resp = http::post(&url, &body, "application/json", &[])
            .map_err(|e| Error::Provider(format!("UnoEuro add TXT: {e}")))?;
        if !resp.body.contains("\"status\": 200") {
            return Err(Error::Provider(format!("UnoEuro add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = match self.get_root(domain) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let url = format!("https://api.simply.com/1/{}/{}/my/products/{zone}/dns/records", self.user, self.key);
        let resp = match http::get(&url, &[]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if !resp.body.contains("\"status\": 200") {
            return Ok(());
        }
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records = v.get("records").and_then(|r| r.as_array())
            .or_else(|| v.as_array());
        if let Some(arr) = records {
            for record in arr {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("data").and_then(|d| d.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("record_id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://api.simply.com/1/{}/{}/my/products/{zone}/dns/records/{id}", self.user, self.key);
                        let _ = http::delete(&del_url, &[]);
                    }
                }
            }
        }
        Ok(())
    }
}

impl Unoeuro {
    fn get_root(&self, domain: &str) -> Result<String, Error> {
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 1..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                continue;
            }
            let url = format!("https://api.simply.com/1/{}/{}/my/products/{h}/dns/records", self.user, self.key);
            let resp = http::get(&url, &[])
                .map_err(|e| Error::Provider(format!("UnoEuro zone: {e}")))?;
            if resp.body.contains("\"status\": 200") {
                return Ok(h);
            }
        }
        Err(Error::Provider(format!("UnoEuro: zone not found for {domain}")))
    }
}
