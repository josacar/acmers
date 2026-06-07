use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Godaddy {
    key: String,
    secret: String,
}

impl DnsProvider for Godaddy {
    fn slug() -> &'static str {
        "gd"
    }

    fn env_vars() -> &'static [&'static str] {
        &["GD_Key", "GD_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let key = env.get("GD_Key")
            .ok_or_else(|| Error::Config("GD_Key required".into()))?
            .clone();
        let secret = env.get("GD_Secret")
            .ok_or_else(|| Error::Config("GD_Secret required".into()))?
            .clone();
        Ok(Box::new(Godaddy { key, secret }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("sso-key {}:{}", self.key, self.secret);
        let record_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];

        let list_url = format!("https://api.godaddy.com/v1/domains/{domain}/records");
        let resp = http::get(&list_url, headers)
            .map_err(|e| Error::Provider(format!("GD list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("GD list response: {e}")))?;

        let mut records: Vec<Value> = v.as_array().cloned().unwrap_or_default();

        records.retain(|r| {
            !(r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                && r.get("name").and_then(|n| n.as_str()) == Some(record_name))
        });

        records.push(serde_json::json!({
            "type": "TXT",
            "name": record_name,
            "data": value,
            "ttl": 600,
        }));

        let body = serde_json::to_vec(&records).unwrap();
        http::put(&list_url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("GD add TXT: {e}")))?;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let auth = format!("sso-key {}:{}", self.key, self.secret);
        let record_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];

        let list_url = format!("https://api.godaddy.com/v1/domains/{domain}/records");
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let mut records: Vec<Value> = v.as_array().cloned().unwrap_or_default();
        records.retain(|r| {
            !(r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                && r.get("name").and_then(|n| n.as_str()) == Some(record_name))
        });

        let body = serde_json::to_vec(&records).unwrap();
        let _ = http::put(&list_url, &body, "application/json", headers);
        Ok(())
    }
}
