use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://public-api.virakcloud.com/dns";

pub struct Virakcloud {
    token: String,
}

impl DnsProvider for Virakcloud {
    fn slug() -> &'static str {
        "virakcloud"
    }

    fn env_vars() -> &'static [&'static str] {
        &["VIRAKCLOUD_API_TOKEN"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("VIRAKCLOUD_API_TOKEN")
            .ok_or_else(|| Error::Config("VIRAKCLOUD_API_TOKEN required".into()))?
            .clone();
        Ok(Box::new(Virakcloud { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let fulldomain = format!("{name}.{domain}");

        let zone = self.resolve_zone(&fulldomain, &auth)?;

        let body = serde_json::json!({
            "record": fulldomain,
            "type": "TXT",
            "ttl": 3600,
            "content": value,
        });
        let url = format!("{BASE_URL}/domains/{zone}/records");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("virakcloud add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("virakcloud add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let fulldomain = format!("{name}.{domain}");

        let zone = match self.resolve_zone(&fulldomain, &auth) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };

        let list_url = format!("{BASE_URL}/domains/{zone}/records");
        let resp = match http::get(&list_url, &[("Authorization", &auth)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };

        let contentid = match find_content_id(&resp.body, value) {
            Some(id) => id,
            None => return Ok(()),
        };

        let del_url = format!("{BASE_URL}/domains/{zone}/records/{fulldomain}/TXT/{contentid}");
        let _ = http::delete(&del_url, &[("Authorization", &auth)]);
        Ok(())
    }
}

impl Virakcloud {
    fn resolve_zone(&self, fulldomain: &str, auth: &str) -> Result<String, Error> {
        let parts: Vec<&str> = fulldomain.split('.').collect();
        let start = if fulldomain.starts_with("_acme-challenge.") { 2 } else { 1 };

        for i in start..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                break;
            }
            let url = format!("{BASE_URL}/domains/{h}");
            match http::get(&url, &[("Authorization", auth)]) {
                Ok(resp) => {
                    if resp.status < 400 {
                        if let Ok(v) = serde_json::from_str::<Value>(&resp.body) {
                            if v.get("name").is_some() {
                                return Ok(h);
                            }
                        }
                    }
                }
                Err(_) => continue,
            }
        }
        Err(Error::Provider("virakcloud: could not resolve zone".into()))
    }
}

fn find_content_id(body: &str, txtvalue: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    find_content_id_recursive(&v, txtvalue)
}

fn find_content_id_recursive(v: &Value, txtvalue: &str) -> Option<String> {
    match v {
        Value::Object(map) => {
            if let (Some(id), Some(raw)) = (map.get("id"), map.get("content_raw")) {
                if raw.as_str() == Some(txtvalue) {
                    return id.as_str().map(|s| s.to_string());
                }
            }
            for val in map.values() {
                if let Some(found) = find_content_id_recursive(val, txtvalue) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(arr) => {
            for item in arr {
                if let Some(found) = find_content_id_recursive(item, txtvalue) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}
