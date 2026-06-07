use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Variomedia {
    basic_auth: String,
}

impl DnsProvider for Variomedia {
    fn slug() -> &'static str {
        "variomedia"
    }

    fn env_vars() -> &'static [&'static str] {
        &["VARIOMEDIA_Email", "VARIOMEDIA_Token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let email = env.get("VARIOMEDIA_Email")
            .ok_or_else(|| Error::Config("VARIOMEDIA_Email required".into()))?
            .clone();
        let token = env.get("VARIOMEDIA_Token")
            .ok_or_else(|| Error::Config("VARIOMEDIA_Token required".into()))?
            .clone();
        let creds = format!("{email}:{token}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Variomedia { basic_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let domain_id = self.resolve_domain(domain, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "name": name,
            "data": value,
            "ttl": 120,
        })).unwrap();
        let url = format!("https://api.variomedia.de/domains/{domain_id}/dns-records");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("Variomedia add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Variomedia add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let domain_id = match self.resolve_domain(domain, headers) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let list_url = format!("https://api.variomedia.de/domains/{domain_id}/dns-records");
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records: Option<&Vec<Value>> = v.as_array()
            .or_else(|| v.get("data").and_then(|d| d.as_array()));
        if let Some(arr) = records {
            for record in arr {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("data").and_then(|d| d.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://api.variomedia.de/domains/{domain_id}/dns-records/{id}");
                        let _ = http::delete(&del_url, headers);
                    }
                }
            }
        }
        Ok(())
    }
}

impl Variomedia {
    fn resolve_domain(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let resp = http::get("https://api.variomedia.de/domains", headers)
            .map_err(|e| Error::Provider(format!("Variomedia list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Variomedia domains: {e}")))?;
        let domains: Option<&Vec<Value>> = v.as_array()
            .or_else(|| v.get("data").and_then(|d| d.as_array()));
        if let Some(arr) = domains {
            for d in arr {
                if let Some(name) = d.get("domain").and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        if let Some(id) = d.get("id").and_then(|i| {
                            if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                        }) {
                            return Ok(id);
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("domain not found: {domain}")))
    }
}
