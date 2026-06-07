use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Constellix {
    api_key: String,
}

impl DnsProvider for Constellix {
    fn slug() -> &'static str {
        "constellix"
    }

    fn env_vars() -> &'static [&'static str] {
        &["CONSTELLIX_Key", "CONSTELLIX_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("CONSTELLIX_Key")
            .ok_or_else(|| Error::Config("CONSTELLIX_Key required".into()))?.clone();
        Ok(Box::new(Constellix { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = self.resolve_zone(domain)?;
        let url = format!("https://api.dns.constellix.com/v1/domains/{domain_id}/records/txt");
        let body = serde_json::json!({
            "name": name,
            "value": [{"value": value, "enabled": true}],
            "ttl": 120,
            "roundRobin": [],
        });
        http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("x-cnsdns-apiKey", &self.api_key)])
            .map_err(|e| Error::Provider(format!("constellix add TXT: {e}")))?;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = self.resolve_zone(domain)?;
        let record_id = self.find_record_id(&domain_id, name, value)?;
        if let Some(id) = record_id {
            let url = format!("https://api.dns.constellix.com/v1/domains/{domain_id}/records/txt/{id}");
            http::delete(&url, &[("x-cnsdns-apiKey", &self.api_key)])
                .map_err(|e| Error::Provider(format!("constellix delete TXT: {e}")))?;
        }
        Ok(())
    }
}

impl Constellix {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let resp = http::get("https://api.dns.constellix.com/v1/domains", &[("x-cnsdns-apiKey", &self.api_key)])
            .map_err(|e| Error::Provider(format!("constellix list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("constellix domains: {e}")))?;
        let domains: Option<&Vec<Value>> = v.as_array()
            .or_else(|| v.get("data").and_then(|d| d.as_array()));
        if let Some(arr) = domains {
            for d in arr {
                if let Some(nm) = d.get("name").and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        if let Some(id) = value_to_string(d.get("id")) {
                            return Ok(id);
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }

    fn find_record_id(&self, domain_id: &str, name: &str, value: &str) -> Result<Option<String>, Error> {
        let url = format!("https://api.dns.constellix.com/v1/domains/{domain_id}/records/txt");
        let resp = http::get(&url, &[("x-cnsdns-apiKey", &self.api_key)])
            .map_err(|e| Error::Provider(format!("constellix list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("constellix records: {e}")))?;
        let records: Option<&Vec<Value>> = v.as_array()
            .or_else(|| v.get("data").and_then(|d| d.as_array()));
        if let Some(arr) = records {
            for r in arr {
                if r.get("name").and_then(|n| n.as_str()) == Some(name) {
                    if let Some(vals) = r.get("value").and_then(|n| n.as_array()) {
                        for val_item in vals {
                            if val_item.get("value").and_then(|n| n.as_str()) == Some(value) {
                                if let Some(id) = value_to_string(r.get("id")) {
                                    return Ok(Some(id));
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

fn value_to_string(v: Option<&Value>) -> Option<String> {
    v.and_then(|v| v.as_str().map(|s| s.to_string())
        .or_else(|| v.as_i64().map(|i| i.to_string())))
}
