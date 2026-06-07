use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct IonosCloud {
    token: String,
}

impl DnsProvider for IonosCloud {
    fn slug() -> &'static str {
        "ionos_cloud"
    }

    fn env_vars() -> &'static [&'static str] {
        &["IONOS_CLOUD_TOKEN"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("IONOS_CLOUD_TOKEN")
            .ok_or_else(|| Error::Config("IONOS_CLOUD_TOKEN required".into()))?
            .clone();
        Ok(Box::new(IonosCloud { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let auth = format!("Bearer {}", self.token);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let body = serde_json::json!({
            "name": name,
            "type": "TXT",
            "content": value,
            "ttl": 120,
        });
        let url = format!("https://dns.de-fra.ionos.com/zones/{zone_id}/records");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("ionos_cloud add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("ionos_cloud add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = match self.resolve_zone(domain) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let auth = format!("Bearer {}", self.token);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let url = format!("https://dns.de-fra.ionos.com/zones/{zone_id}/records");
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
                        let del_url = format!("https://dns.de-fra.ionos.com/zones/{zone_id}/records/{id}");
                        let _ = http::delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl IonosCloud {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let auth = format!("Bearer {}", self.token);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let resp = http::get("https://dns.de-fra.ionos.com/zones", headers)
            .map_err(|e| Error::Provider(format!("ionos_cloud list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("ionos_cloud parse zones: {e}")))?;
        if let Some(arr) = v.as_array() {
            for z in arr {
                if let Some(nm) = z.get("name").and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        if let Some(id) = z.get("id").and_then(|i| {
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
