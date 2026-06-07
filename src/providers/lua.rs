use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Luadns {
    basic_auth: String,
}

impl DnsProvider for Luadns {
    fn slug() -> &'static str {
        "lua"
    }

    fn env_vars() -> &'static [&'static str] {
        &["LUA_Key", "LUA_Email"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let email = env.get("LUA_Email")
            .ok_or_else(|| Error::Config("LUA_Email required".into()))?
            .clone();
        let key = env.get("LUA_Key")
            .ok_or_else(|| Error::Config("LUA_Key required".into()))?
            .clone();
        let creds = format!("{email}:{key}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Luadns { basic_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let body = serde_json::json!({
            "type": "TXT",
            "name": name,
            "content": format!("\"{value}\""),
            "ttl": 120,
        });
        let url = format!("https://api.luadns.com/v1/zones/{zone_id}/records");
        http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("lua add TXT: {e}")))?;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = match self.resolve_zone(domain) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let url = format!("https://api.luadns.com/v1/zones/{zone_id}/records");
        let resp = match http::get(&url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let quoted_value = format!("\"{value}\"");
        if let Some(records) = v.as_array() {
            for r in records {
                if r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && r.get("name").and_then(|n| n.as_str()) == Some(name)
                    && r.get("content").and_then(|c| c.as_str()) == Some(&quoted_value)
                {
                    if let Some(id) = r.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://api.luadns.com/v1/zones/{zone_id}/records/{id}");
                        let _ = http::delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Luadns {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let resp = http::get("https://api.luadns.com/v1/zones", headers)
            .map_err(|e| Error::Provider(format!("lua list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("lua parse zones: {e}")))?;
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
