use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Arvan {
    api_key: String,
}

impl DnsProvider for Arvan {
    fn slug() -> &'static str {
        "arvan"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ARVAN_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("ARVAN_API_KEY")
            .ok_or_else(|| Error::Config("ARVAN_API_KEY required".into()))?.clone();
        Ok(Box::new(Arvan { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = self.resolve_domain(domain)?;
        let url = format!("https://napi.arvancloud.ir/cdn/4.0/domains/{domain_id}/dns-records");
        let body = serde_json::json!({
            "type": "TXT",
            "name": name,
            "value": {"text": value},
            "ttl": 120,
        });
        let auth = format!("ApiKey {}", self.api_key);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("arvan add TXT: {e}")))?;
        if resp.status >= 300 {
            return Err(Error::Provider(format!("arvan add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = match self.resolve_domain(domain) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("warning: arvan cleanup zone not found: {e}");
                return Ok(());
            }
        };
        match self.find_record(&domain_id, name, value) {
            Ok(Some(record_id)) => {
                let url = format!("https://napi.arvancloud.ir/cdn/4.0/domains/{domain_id}/dns-records/{record_id}");
                let auth = format!("ApiKey {}", self.api_key);
                http::delete(&url, &[("Authorization", &auth)]).ok();
                Ok(())
            }
            Ok(None) => Ok(()),
            Err(e) => {
                eprintln!("warning: arvan cleanup failed: {e}");
                Ok(())
            }
        }
    }
}

impl Arvan {
    fn resolve_domain(&self, domain: &str) -> Result<String, Error> {
        let auth = format!("ApiKey {}", self.api_key);
        let resp = http::get("https://napi.arvancloud.ir/cdn/4.0/domains", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("arvan list domains: {e}")))?;
        if resp.status >= 300 {
            return Err(Error::Provider(format!("arvan list domains: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("arvan domains: {e}")))?;
        if let Some(domains) = v.get("data").and_then(|d| d.as_array()) {
            for d in domains {
                if let Some(dname) = d.get("domain").and_then(|n| n.as_str()) {
                    if domain == dname || domain.ends_with(&format!(".{dname}")) {
                        if let Some(id) = d.get("id") {
                            let id_str = if id.is_number() {
                                id.as_i64().map(|n| n.to_string())
                            } else {
                                id.as_str().map(|s| s.to_string())
                            };
                            if let Some(id_str) = id_str {
                                return Ok(id_str);
                            }
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("domain not found for {domain}")))
    }

    fn find_record(&self, domain_id: &str, name: &str, value: &str) -> Result<Option<String>, Error> {
        let url = format!("https://napi.arvancloud.ir/cdn/4.0/domains/{domain_id}/dns-records");
        let auth = format!("ApiKey {}", self.api_key);
        let resp = http::get(&url, &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("arvan list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("arvan records: {e}")))?;
        if let Some(records) = v.get("data").and_then(|d| d.as_array()) {
            for record in records {
                let rtype = record.get("type").and_then(|t| t.as_str()).unwrap_or("");
                let rname = record.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if rtype == "TXT" && rname == name {
                    if let Some(val) = record.get("value").and_then(|v| v.get("text")).and_then(|t| t.as_str()) {
                        if val == value {
                            if let Some(id) = record.get("id") {
                                let id_str = if id.is_number() {
                                    id.as_i64().map(|n| n.to_string())
                                } else {
                                    id.as_str().map(|s| s.to_string())
                                };
                                if let Some(id_str) = id_str {
                                    return Ok(Some(id_str));
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }
}
