use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Njalla {
    token: String,
}

impl DnsProvider for Njalla {
    fn slug() -> &'static str {
        "njalla"
    }

    fn env_vars() -> &'static [&'static str] {
        &["NJALLA_Token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("NJALLA_Token")
            .ok_or_else(|| Error::Config("NJALLA_Token required".into()))?.clone();
        Ok(Box::new(Njalla { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = self.resolve_zone(domain)?;
        let url = format!("https://njal.la/api/1/domains/{domain_id}/records/");
        let body = serde_json::json!({
            "type": "TXT",
            "name": name,
            "content": value,
            "ttl": 120,
        });
        let auth = format!("Njalla {}", self.token);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("njalla add TXT: {e}")))?;
        if resp.status >= 300 {
            return Err(Error::Provider(format!("njalla add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = self.resolve_zone(domain)?;
        let record_id = self.find_record_id(&domain_id, name, value)?;
        if let Some(id) = record_id {
            let url = format!("https://njal.la/api/1/domains/{domain_id}/records/{id}/");
            let auth = format!("Njalla {}", self.token);
            http::delete(&url, &[("Authorization", &auth)])
                .map_err(|e| Error::Provider(format!("njalla delete TXT: {e}")))?;
        }
        Ok(())
    }
}

impl Njalla {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let auth = format!("Njalla {}", self.token);
        let resp = http::get("https://njal.la/api/1/domains/", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("njalla list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("njalla domains: {e}")))?;
        let domains: Option<&Vec<Value>> = v.as_array()
            .or_else(|| v.get("domains").and_then(|d| d.as_array()));
        if let Some(arr) = domains {
            for d in arr {
                if let Some(nm) = d.get("name").and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        if let Some(id) = d.get("id").and_then(|i| i.as_str()) {
                            return Ok(id.to_string());
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }

    fn find_record_id(&self, domain_id: &str, name: &str, value: &str) -> Result<Option<String>, Error> {
        let auth = format!("Njalla {}", self.token);
        let url = format!("https://njal.la/api/1/domains/{domain_id}/records/");
        let resp = http::get(&url, &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("njalla list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("njalla records: {e}")))?;
        let records: Option<&Vec<Value>> = v.as_array()
            .or_else(|| v.get("records").and_then(|r| r.as_array()));
        if let Some(arr) = records {
            for r in arr {
                if r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && r.get("name").and_then(|n| n.as_str()) == Some(name)
                    && r.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = r.get("id").and_then(|i| i.as_i64()) {
                        return Ok(Some(id.to_string()));
                    }
                }
            }
        }
        Ok(None)
    }
}
