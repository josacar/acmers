use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Porkbun {
    api_key: String,
    secret_api_key: String,
}

impl DnsProvider for Porkbun {
    fn slug() -> &'static str {
        "porkbun"
    }

    fn env_vars() -> &'static [&'static str] {
        &["PORKBUN_API_KEY", "PORKBUN_SECRET_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("PORKBUN_API_KEY")
            .ok_or_else(|| Error::Config("PORKBUN_API_KEY required".into()))?
            .clone();
        let secret_api_key = env.get("PORKBUN_SECRET_API_KEY")
            .ok_or_else(|| Error::Config("PORKBUN_SECRET_API_KEY required".into()))?
            .clone();
        Ok(Box::new(Porkbun { api_key, secret_api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let body = serde_json::to_vec(&serde_json::json!({
            "apikey": self.api_key,
            "secretapikey": self.secret_api_key,
            "type": "TXT",
            "name": name,
            "content": value,
            "ttl": "120",
        })).unwrap();
        let url = format!("https://api.porkbun.com/api/json/v3/dns/create/{domain}");
        let resp = http::post(&url, &body, "application/json", &[])
            .map_err(|e| Error::Provider(format!("Porkbun add TXT: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Porkbun response: {e}")))?;
        if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
            if v.get("status").and_then(|s| s.as_str()) != Some("SUCCESS") {
                return Err(Error::Provider(format!("Porkbun add TXT: {msg}")));
            }
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let list_body = serde_json::to_vec(&serde_json::json!({
            "apikey": self.api_key,
            "secretapikey": self.secret_api_key,
        })).unwrap();
        let list_url = format!("https://api.porkbun.com/api/json/v3/dns/retrieve/{domain}");
        let resp = match http::post(&list_url, &list_body, "application/json", &[]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(records) = v.get("records").and_then(|r| r.as_array()) {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| i.as_str()) {
                        let del_url = format!("https://api.porkbun.com/api/json/v3/dns/delete/{domain}/{id}");
                        let del_body = serde_json::to_vec(&serde_json::json!({
                            "apikey": self.api_key,
                            "secretapikey": self.secret_api_key,
                        })).unwrap();
                        let _ = http::post(&del_url, &del_body, "application/json", &[]);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}
