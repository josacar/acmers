use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Dnshome {
    basic_auth: String,
}

impl DnsProvider for Dnshome {
    fn slug() -> &'static str {
        "dnshome"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DNSHOME_Username", "DNSHOME_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("DNSHOME_Username")
            .ok_or_else(|| Error::Config("DNSHOME_Username required".into()))?
            .clone();
        let password = env.get("DNSHOME_Password")
            .ok_or_else(|| Error::Config("DNSHOME_Password required".into()))?
            .clone();
        let creds = format!("{username}:{password}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Dnshome { basic_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let body = serde_json::json!({
            "type": "TXT",
            "name": name,
            "content": value,
            "ttl": 120,
        });
        let url = format!("https://www.dnshome.de/api/v1/domains/{domain}/records");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("dnshome add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("dnshome add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let list_url = format!("https://www.dnshome.de/api/v1/domains/{domain}/records");
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(records) = v.as_array() {
            for r in records {
                if r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && r.get("name").and_then(|n| n.as_str()) == Some(name)
                    && r.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = r.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://www.dnshome.de/api/v1/domains/{domain}/records/{id}");
                        let _ = http::delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}
