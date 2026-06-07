use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Zilore {
    basic_auth: String,
}

impl DnsProvider for Zilore {
    fn slug() -> &'static str {
        "zilore"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ZILORE_Key", "ZILORE_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let key = env.get("ZILORE_Key")
            .ok_or_else(|| Error::Config("ZILORE_Key required".into()))?
            .clone();
        let secret = env.get("ZILORE_Secret")
            .ok_or_else(|| Error::Config("ZILORE_Secret required".into()))?
            .clone();
        let creds = format!("{key}:{secret}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Zilore { basic_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let body = serde_json::json!({
            "type": "TXT",
            "name": name,
            "content": value,
            "ttl": 120,
        });
        let url = format!("https://api.zilore.com/dns/v1/domains/{zone_id}/records");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("zilore add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("zilore add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = match self.resolve_zone(domain) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let url = format!("https://api.zilore.com/dns/v1/domains/{zone_id}/records");
        let resp = match http::get(&url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records = if let Some(arr) = v.as_array() {
            Some(arr)
        } else {
            v.get("data").and_then(|d| d.as_array())
        };
        if let Some(arr) = records {
            for r in arr {
                if r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && r.get("name").and_then(|n| n.as_str()) == Some(name)
                    && r.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = r.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://api.zilore.com/dns/v1/domains/{zone_id}/records/{id}");
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
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let resp = http::get("https://api.zilore.com/dns/v1/domains", headers)
            .map_err(|e| Error::Provider(format!("zilore list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("zilore parse domains: {e}")))?;
        let domains = if let Some(arr) = v.as_array() {
            Some(arr)
        } else {
            v.get("data").and_then(|d| d.as_array())
        };
        if let Some(arr) = domains {
            for d in arr {
                if let Some(nm) = d.get("name").and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        if let Some(id) = d.get("id").and_then(|i| {
                            if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                        }) {
                            return Ok(id);
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
