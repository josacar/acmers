use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Euserv {
    basic_auth: String,
}

impl DnsProvider for Euserv {
    fn slug() -> &'static str {
        "euserv"
    }

    fn env_vars() -> &'static [&'static str] {
        &["EUSERV_Username", "EUSERV_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("EUSERV_Username")
            .ok_or_else(|| Error::Config("EUSERV_Username required".into()))?
            .clone();
        let password = env.get("EUSERV_Password")
            .ok_or_else(|| Error::Config("EUSERV_Password required".into()))?
            .clone();
        let creds = format!("{username}:{password}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Euserv { basic_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "name": name,
            "data": value,
            "ttl": 120,
        })).unwrap();
        let url = format!("https://api.euserv.net/v1/domains/{domain}/records");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("EUServ add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("EUServ add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let list_url = format!("https://api.euserv.net/v1/domains/{domain}/records");
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records: Option<&Vec<Value>> = if let Some(arr) = v.as_array() {
            Some(arr)
        } else {
            v.get("records").and_then(|r| r.as_array())
        };
        if let Some(arr) = records {
            for record in arr {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("data").and_then(|d| d.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://api.euserv.net/v1/domains/{domain}/records/{id}");
                        let _ = http::delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}
