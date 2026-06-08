use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://www.lima-city.de/usercp";

pub struct Limacity {
    api_key: String,
}

impl Limacity {
    fn auth_header(&self) -> String {
        let creds = format!("api:{}", self.api_key);
        let encoded = base64::encode_std(creds.as_bytes());
        format!("Basic {encoded}")
    }

    fn get_domain_id(&self, fulldomain: &str) -> Result<String, Error> {
        let url = format!("{BASE_URL}/domains.json");
        let resp = http::get(&url, &[("Authorization", &self.auth_header())])
            .map_err(|e| Error::Provider(format!("limacity domains: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("limacity domains: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("limacity domains: {e}")))?;

        let domains = v.get("domains").and_then(|d| d.as_array())
            .ok_or_else(|| Error::Provider("limacity: no domains in response".into()))?;

        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 2..=parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                break;
            }
            for domain in domains {
                if domain.get("unicode_fqdn").and_then(|f| f.as_str()) == Some(&h) {
                    if let Some(id) = domain.get("id") {
                        let id_str = if id.is_u64() {
                            id.as_u64().unwrap().to_string()
                        } else if id.is_i64() {
                            id.as_i64().unwrap().to_string()
                        } else if id.is_string() {
                            id.as_str().unwrap().to_string()
                        } else {
                            continue;
                        };
                        return Ok(id_str);
                    }
                }
            }
        }
        Err(Error::Provider(format!("limacity: no domain found for {fulldomain}")))
    }
}

impl DnsProvider for Limacity {
    fn slug() -> &'static str {
        "limacity"
    }

    fn env_vars() -> &'static [&'static str] {
        &["LIMACITY_APIKEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("LIMACITY_APIKEY")
            .ok_or_else(|| Error::Config("LIMACITY_APIKEY required".into()))?
            .clone();
        Ok(Box::new(Limacity { api_key }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = self.get_domain_id(name)?;
        let auth = self.auth_header();

        let body = serde_json::json!({
            "nameserver_record": {
                "name": name,
                "type": "TXT",
                "content": value,
                "ttl": 60
            }
        });
        let url = format!("{BASE_URL}/domains/{domain_id}/records.json");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("limacity add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("limacity add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("limacity response: {e}")))?;
        if v.get("status").and_then(|s| s.as_str()) != Some("ok") {
            let msg = v.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error");
            return Err(Error::Provider(format!("limacity add TXT: {msg}")));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, _value: &str) -> ProviderResult {
        let domain_id = match self.get_domain_id(name) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let auth = self.auth_header();

        let list_url = format!("{BASE_URL}/domains/{domain_id}/records.json");
        let resp = match http::get(&list_url, &[("Authorization", &auth)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let records = v.get("records").and_then(|r| r.as_array())
            .or_else(|| v.as_array());
        if let Some(records) = records {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                {
                    if let Some(id) = record_id(record) {
                        let del_url = format!("{BASE_URL}/domains/{domain_id}/records/{id}");
                        let _ = http::delete(&del_url, &[("Authorization", &auth)]);
                    }
                }
            }
        }
        Ok(())
    }
}

fn record_id(v: &Value) -> Option<String> {
    v.get("id").and_then(|i| {
        if i.is_string() { i.as_str().map(|s| s.to_string()) }
        else if i.is_u64() { i.as_u64().map(|n| n.to_string()) }
        else if i.is_i64() { i.as_i64().map(|n| n.to_string()) }
        else { None }
    })
}
