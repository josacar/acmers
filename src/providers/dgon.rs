use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Digitalocean {
    token: String,
}

impl DnsProvider for Digitalocean {
    fn slug() -> &'static str {
        "dgon"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DO_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("DO_API_KEY")
            .ok_or_else(|| Error::Config("DO_API_KEY required".into()))?
            .clone();
        Ok(Box::new(Digitalocean { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let zone_name = self.resolve_zone(domain, &auth)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "name": name,
            "data": value,
            "ttl": 120,
        })).unwrap();
        let url = format!("https://api.digitalocean.com/v2/domains/{zone_name}/records");
        let resp = http::post(&url, &body, "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("DO add TXT: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("DO response: {e}")))?;
        if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
            if resp.status >= 400 {
                return Err(Error::Provider(format!("DO add TXT: {msg}")));
            }
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let zone_name = match self.resolve_zone(domain, &auth) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let list_url = format!("https://api.digitalocean.com/v2/domains/{zone_name}/records?type=TXT");
        let resp = match http::get(&list_url, &[("Authorization", &auth)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(records) = v.get("domain_records").and_then(|r| r.as_array()) {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("data").and_then(|d| d.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| i.as_i64()) {
                        let del_url = format!("https://api.digitalocean.com/v2/domains/{zone_name}/records/{id}");
                        let _ = http::delete(&del_url, &[("Authorization", &auth)]);
                    }
                }
            }
        }
        Ok(())
    }
}

impl Digitalocean {
    fn resolve_zone(&self, domain: &str, auth: &str) -> Result<String, Error> {
        let resp = http::get("https://api.digitalocean.com/v2/domains", &[("Authorization", auth)])
            .map_err(|e| Error::Provider(format!("DO list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("DO domains: {e}")))?;
        if let Some(domains) = v.get("domains").and_then(|d| d.as_array()) {
            for d in domains {
                if let Some(name) = d.get("name").and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        return Ok(name.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
