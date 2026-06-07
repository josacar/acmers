use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Websupport {
    basic_auth: String,
}

impl DnsProvider for Websupport {
    fn slug() -> &'static str {
        "websupport"
    }

    fn env_vars() -> &'static [&'static str] {
        &["WEBSUPPORT_Key", "WEBSUPPORT_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let key = env.get("WEBSUPPORT_Key")
            .ok_or_else(|| Error::Config("WEBSUPPORT_Key required".into()))?
            .clone();
        let secret = env.get("WEBSUPPORT_Secret")
            .ok_or_else(|| Error::Config("WEBSUPPORT_Secret required".into()))?
            .clone();
        let creds = format!("{key}:{secret}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Websupport { basic_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let domain_id = self.resolve_domain(domain, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "name": name,
            "content": value,
            "ttl": 120,
        })).unwrap();
        let url = format!("https://rest.websupport.sk/v1/admin/domains/{domain_id}/zones/records");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("WebSupport add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("WebSupport add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let domain_id = match self.resolve_domain(domain, headers) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let list_url = format!("https://rest.websupport.sk/v1/admin/domains/{domain_id}/zones/records");
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records: Option<&Vec<Value>> = v.as_array()
            .or_else(|| v.get("items").and_then(|i| i.as_array()));
        if let Some(arr) = records {
            for record in arr {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://rest.websupport.sk/v1/admin/domains/{domain_id}/zones/records/{id}");
                        let _ = http::delete(&del_url, headers);
                    }
                }
            }
        }
        Ok(())
    }
}

impl Websupport {
    fn resolve_domain(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let resp = http::get("https://rest.websupport.sk/v1/admin/domains", headers)
            .map_err(|e| Error::Provider(format!("WebSupport list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("WebSupport domains: {e}")))?;
        let domains: Option<&Vec<Value>> = v.as_array()
            .or_else(|| v.get("items").and_then(|i| i.as_array()));
        if let Some(arr) = domains {
            for d in arr {
                if let Some(name) = d.get("name").and_then(|n| n.as_str()) {
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
