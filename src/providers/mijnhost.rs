use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Mijnhost {
    api_key: String,
}

impl DnsProvider for Mijnhost {
    fn slug() -> &'static str {
        "mijnhost"
    }

    fn env_vars() -> &'static [&'static str] {
        &["MIJNHOST_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("MIJNHOST_API_KEY")
            .ok_or_else(|| Error::Config("MIJNHOST_API_KEY required".into()))?
            .clone();
        Ok(Box::new(Mijnhost { api_key }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("API-Key", &self.api_key)];
        let root = self.resolve_domain(name, headers)?;
        let url = format!("https://mijn.host/api/v2/domains/{root}/dns");
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("Mijnhost get DNS: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Mijnhost get DNS: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Mijnhost parse DNS: {e}")))?;
        let mut records = v.get("records")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();
        records.push(serde_json::json!({
            "type": "TXT",
            "name": format!("{name}."),
            "value": value,
            "ttl": 300,
        }));
        let body = serde_json::to_vec(&serde_json::json!({
            "records": records,
        })).unwrap();
        let resp = http::put(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("Mijnhost add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Mijnhost add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("API-Key", &self.api_key)];
        let root = match self.resolve_domain(name, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let url = format!("https://mijn.host/api/v2/domains/{root}/dns");
        let resp = match http::get(&url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records = v.get("records")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();
        let filtered: Vec<Value> = records.into_iter()
            .filter(|r| {
                !(r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && r.get("value").and_then(|v| v.as_str()) == Some(value))
            })
            .collect();
        let body = serde_json::to_vec(&serde_json::json!({
            "records": filtered,
        })).unwrap();
        let resp = match http::put(&url, &body, "application/json", headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Mijnhost remove TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }
}

impl Mijnhost {
    fn resolve_domain(&self, fulldomain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let resp = http::get("https://mijn.host/api/v2/domains", headers)
            .map_err(|e| Error::Provider(format!("Mijnhost list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Mijnhost domains: {e}")))?;
        if let Some(domains) = v.get("data").and_then(|d| d.as_array()) {
            for d in domains {
                if let Some(domain_name) = d.get("domain").and_then(|n| n.as_str()) {
                    if fulldomain.contains(domain_name) {
                        return Ok(domain_name.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("domain not found for {fulldomain}")))
    }
}
