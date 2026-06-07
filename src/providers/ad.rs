use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Alwaysdata {
    basic_auth: String,
}

impl DnsProvider for Alwaysdata {
    fn slug() -> &'static str {
        "ad"
    }

    fn env_vars() -> &'static [&'static str] {
        &["AD_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("AD_API_KEY")
            .ok_or_else(|| Error::Config("AD_API_KEY required".into()))?
            .clone();
        let creds = format!("{api_key}:");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Alwaysdata { basic_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let domain_id = self.resolve_domain(domain, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "name": name,
            "value": value,
            "ttl": 120,
        })).unwrap();
        let url = format!("https://api.alwaysdata.com/v1/domain/{domain_id}/record");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("Alwaysdata add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Alwaysdata add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let domain_id = match self.resolve_domain(domain, headers) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };
        let list_url = format!("https://api.alwaysdata.com/v1/domain/{domain_id}/record");
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
                    && record.get("value").and_then(|v| v.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://api.alwaysdata.com/v1/domain/{domain_id}/record/{id}");
                        let _ = http::delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Alwaysdata {
    fn resolve_domain(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let resp = http::get("https://api.alwaysdata.com/v1/domain", headers)
            .map_err(|e| Error::Provider(format!("Alwaysdata list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Alwaysdata domains: {e}")))?;
        let domains = v.get("data").and_then(|d| d.as_array())
            .or_else(|| v.as_array());
        if let Some(domains) = domains {
            for d in domains {
                if d.get("name").and_then(|n| n.as_str()) == Some(domain)
                {
                    if let Some(id) = d.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        return Ok(id);
                    }
                }
            }
        }
        Err(Error::Provider(format!("domain not found for {domain}")))
    }
}
