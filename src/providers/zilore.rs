use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Zilore {
    api_key: String,
}

impl DnsProvider for Zilore {
    fn slug() -> &'static str {
        "zilore"
    }

    fn env_vars() -> &'static [&'static str] {
        &["Zilore_Key"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("Zilore_Key")
            .ok_or_else(|| Error::Config("Zilore_Key required".into()))?
            .clone();
        Ok(Box::new(Zilore { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = self.resolve_zone(domain)?;
        let headers: &[(&str, &str)] = &[("X-Auth-Key", &self.api_key)];
        let url = format!(
            "https://api.zilore.com/dns/v1/domains/{zone}/records?record_type=TXT&record_ttl=600&record_name={name}&record_value=\"{value}\""
        );
        let resp = http::post(&url, b"", "application/json", headers)
            .map_err(|e| Error::Provider(format!("zilore add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("zilore add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = match self.resolve_zone(domain) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let headers: &[(&str, &str)] = &[("X-Auth-Key", &self.api_key)];
        let url = format!(
            "https://api.zilore.com/dns/v1/domains/{zone}/records?search_text={value}&search_record_type=TXT"
        );
        let resp = match http::get(&url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records = v.get("response").and_then(|d| d.as_array())
            .or_else(|| v.as_array());
        if let Some(arr) = records {
            for r in arr {
                if r.get("record_type").and_then(|t| t.as_str()) == Some("TXT")
                    && r.get("record_name").and_then(|n| n.as_str()) == Some(name)
                    && r.get("record_value").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = r.get("record_id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!(
                            "https://api.zilore.com/dns/v1/domains/{zone}/records?record_id={id}"
                        );
                        let _ = http::delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Zilore {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let headers: &[(&str, &str)] = &[("X-Auth-Key", &self.api_key)];
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            let url = format!("https://api.zilore.com/dns/v1/domains?search_text={h}");
            let resp = http::get(&url, headers)
                .map_err(|e| Error::Provider(format!("zilore list domains: {e}")))?;
            if resp.body.contains(&format!("\"{h}\"")) {
                return Ok(h);
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
