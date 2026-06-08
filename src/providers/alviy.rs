use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://cloud.alviy.com/api/v1";

pub struct Alviy {
    auth: String,
}

impl DnsProvider for Alviy {
    fn slug() -> &'static str {
        "alviy"
    }

    fn env_vars() -> &'static [&'static str] {
        &["Alviy_token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("Alviy_token")
            .ok_or_else(|| Error::Config("Alviy_token required".into()))?
            .clone();
        let auth = format!("Bearer {token}");
        Ok(Box::new(Alviy { auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let url = format!("{BASE_URL}/zone/{domain}/domain/{name}/");
        let body = serde_json::json!({
            "content": value,
            "type": "TXT",
        });
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("Authorization", &self.auth)])
            .map_err(|e| Error::Provider(format!("alviy add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("alviy add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let list_url = format!("{BASE_URL}/zone/{domain}/domain/{name}/TXT/");
        let resp = match http::get(&list_url, &[("Authorization", &self.auth)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if resp.status >= 400 {
            return Ok(());
        }
        let uuid = find_uuid_by_value(&resp.body, value);
        let uuid = match uuid {
            Some(u) => u,
            None => return Ok(()),
        };
        let del_url = format!("{BASE_URL}/zone/{domain}/record/{uuid}");
        let body = serde_json::json!({"confirm": 1});
        let _ = http::delete_with_body(&del_url, &serde_json::to_vec(&body).unwrap(),
            "application/json", &[("Authorization", &self.auth)]);
        Ok(())
    }
}

fn find_uuid_by_value(body: &str, value: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    let arr = v.as_array().or_else(|| v.get("data").and_then(|d| d.as_array()))?;
    for record in arr {
        let has_value = record.get("content").and_then(|c| c.as_str()) == Some(value);
        if has_value {
            if let Some(uuid) = record.get("uuid").and_then(|u| u.as_str()) {
                return Some(uuid.to_string());
            }
        }
    }
    None
}
