use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Timeweb {
    token: String,
}

impl DnsProvider for Timeweb {
    fn slug() -> &'static str {
        "timeweb"
    }

    fn env_vars() -> &'static [&'static str] {
        &["TIMEWEB_Token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("TIMEWEB_Token")
            .ok_or_else(|| Error::Config("TIMEWEB_Token required".into()))?
            .clone();
        Ok(Box::new(Timeweb { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let domain_id = self.resolve_domain(domain, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "subdomain": name,
            "value": value,
            "ttl": 120,
        })).unwrap();
        let url = format!("https://api.timeweb.cloud/api/v1/domains/{domain_id}/dns-records");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("Timeweb add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Timeweb add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let domain_id = match self.resolve_domain(domain, headers) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let list_url = format!("https://api.timeweb.cloud/api/v1/domains/{domain_id}/dns-records");
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records: Option<&Vec<Value>> = v.as_array()
            .or_else(|| v.get("dns_records").and_then(|r| r.as_array()))
            .or_else(|| v.get("records").and_then(|r| r.as_array()));
        if let Some(arr) = records {
            for record in arr {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("subdomain").and_then(|n| n.as_str()) == Some(name)
                    && record.get("value").and_then(|v| v.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://api.timeweb.cloud/api/v1/domains/{domain_id}/dns-records/{id}");
                        let _ = http::delete(&del_url, headers);
                    }
                }
            }
        }
        Ok(())
    }
}

impl Timeweb {
    fn resolve_domain(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let resp = http::get("https://api.timeweb.cloud/api/v1/domains", headers)
            .map_err(|e| Error::Provider(format!("Timeweb list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Timeweb domains: {e}")))?;
        let domains: Option<&Vec<Value>> = v.as_array()
            .or_else(|| v.get("domains").and_then(|d| d.as_array()));
        if let Some(arr) = domains {
            for d in arr {
                if let Some(fqdn) = d.get("fqdn").and_then(|n| n.as_str()) {
                    if domain == fqdn || domain.ends_with(&format!(".{fqdn}")) {
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
