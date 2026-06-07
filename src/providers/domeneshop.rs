use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Domeneshop {
    basic_auth: String,
}

impl DnsProvider for Domeneshop {
    fn slug() -> &'static str {
        "domeneshop"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DOMENESHOP_Key", "DOMENESHOP_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let key = env.get("DOMENESHOP_Key")
            .ok_or_else(|| Error::Config("DOMENESHOP_Key required".into()))?
            .clone();
        let secret = env.get("DOMENESHOP_Secret")
            .ok_or_else(|| Error::Config("DOMENESHOP_Secret required".into()))?
            .clone();
        let creds = format!("{key}:{secret}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Domeneshop { basic_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let domain_id = self.resolve_domain(domain, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "host": name,
            "data": value,
            "ttl": 120,
        })).unwrap();
        let url = format!("https://api.domeneshop.no/v0/domains/{domain_id}/dns");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("Domeneshop add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Domeneshop add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let domain_id = match self.resolve_domain(domain, headers) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let list_url = format!("https://api.domeneshop.no/v0/domains/{domain_id}/dns");
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
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("host").and_then(|h| h.as_str()) == Some(name)
                    && record.get("data").and_then(|d| d.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://api.domeneshop.no/v0/domains/{domain_id}/dns/{id}");
                        let _ = http::delete(&del_url, headers);
                    }
                }
            }
        }
        Ok(())
    }
}

impl Domeneshop {
    fn resolve_domain(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let resp = http::get("https://api.domeneshop.no/v0/domains", headers)
            .map_err(|e| Error::Provider(format!("Domeneshop list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Domeneshop domains: {e}")))?;
        if let Some(domains) = v.as_array() {
            for d in domains {
                if let Some(dom) = d.get("domain").and_then(|n| n.as_str()) {
                    if domain == dom || domain.ends_with(&format!(".{dom}")) {
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
