use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Linodev4 {
    token: String,
}

impl DnsProvider for Linodev4 {
    fn slug() -> &'static str {
        "linode_v4"
    }

    fn env_vars() -> &'static [&'static str] {
        &["LINODE_V4_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("LINODE_V4_API_KEY")
            .ok_or_else(|| Error::Config("LINODE_V4_API_KEY required".into()))?
            .clone();
        Ok(Box::new(Linodev4 { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let domain_id = self.resolve_zone(domain, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "name": name,
            "target": value,
            "ttl_sec": 120,
            "priority": 0,
        })).unwrap();
        let url = format!("https://api.linode.com/v4/domains/{domain_id}/records");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("Linode add TXT: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Linode response: {e}")))?;
        if let Some(errors) = v.get("errors").and_then(|e| e.as_array()) {
            if let Some(err) = errors.first().and_then(|e| e.get("reason").and_then(|r| r.as_str())) {
                return Err(Error::Provider(format!("Linode add TXT: {err}")));
            }
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let domain_id = match self.resolve_zone(domain, headers) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };
        let list_url = format!("https://api.linode.com/v4/domains/{domain_id}/records");
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(records) = v.get("data").and_then(|r| r.as_array()) {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("target").and_then(|t| t.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| i.as_i64()) {
                        let del_url = format!("https://api.linode.com/v4/domains/{domain_id}/records/{id}");
                        let _ = http::delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Linodev4 {
    fn resolve_zone(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let resp = http::get("https://api.linode.com/v4/domains", headers)
            .map_err(|e| Error::Provider(format!("Linode list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Linode domains: {e}")))?;
        if let Some(domains) = v.get("data").and_then(|d| d.as_array()) {
            for d in domains {
                if let Some(name) = d.get("domain").and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        if let Some(id) = d.get("id").and_then(|i| i.as_i64()) {
                            return Ok(id.to_string());
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
