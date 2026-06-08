use std::collections::HashMap;
use serde_json::Value;
use ring::hmac;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const API_BASE: &str = "https://rest.active24.cz";

pub struct Active24 {
    api_key: String,
    api_secret: String,
}

impl Active24 {
    fn auth_headers(&self, method: &str, endpoint: &str) -> Vec<(String, String)> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let now = time::OffsetDateTime::now_utc();
        let datez = format!(
            "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
            now.year(), now.month() as u8, now.day(),
            now.hour(), now.minute(), now.second()
        );

        let canonical = format!("{method} {endpoint} {timestamp}");

        let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, self.api_secret.as_bytes());
        let tag = hmac::sign(&key, canonical.as_bytes());
        let signature = crate::base64::hex(tag.as_ref());

        let auth_plain = format!("{}:{}", self.api_key, signature);
        let auth_b64 = crate::base64::encode_std(auth_plain.as_bytes());

        vec![
            ("Date".to_string(), datez),
            ("Accept".to_string(), "application/json".to_string()),
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Authorization".to_string(), format!("Basic {auth_b64}")),
        ]
    }

    fn header_refs(headers: &[(String, String)]) -> Vec<(&str, &str)> {
        headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
    }

    fn rest_get(&self, endpoint: &str) -> Result<http::Response, Error> {
        let headers = self.auth_headers("GET", endpoint);
        let hrefs = Self::header_refs(&headers);
        let url = format!("{API_BASE}{endpoint}");
        http::get(&url, &hrefs)
            .map_err(|e| Error::Provider(format!("Active24 GET {endpoint}: {e}")))
    }

    fn rest_post(&self, endpoint: &str, body: &[u8]) -> Result<http::Response, Error> {
        let headers = self.auth_headers("POST", endpoint);
        let hrefs = Self::header_refs(&headers);
        let url = format!("{API_BASE}{endpoint}");
        http::post(&url, body, "application/json", &hrefs)
            .map_err(|e| Error::Provider(format!("Active24 POST {endpoint}: {e}")))
    }

    fn rest_delete(&self, endpoint: &str) -> Result<http::Response, Error> {
        let headers = self.auth_headers("DELETE", endpoint);
        let hrefs = Self::header_refs(&headers);
        let url = format!("{API_BASE}{endpoint}");
        http::delete(&url, &hrefs)
            .map_err(|e| Error::Provider(format!("Active24 DELETE {endpoint}: {e}")))
    }

    fn resolve_zone(&self, fulldomain: &str) -> Result<(String, String), Error> {
        let resp = self.rest_get("/v1/user/self/service")?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!(
                "Active24 service list: HTTP {} {}", resp.status, resp.body
            )));
        }

        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                return Err(Error::Provider(format!("Active24: cannot find zone for {fulldomain}")));
            }
            if resp.body.contains(&format!("\"{h}\"")) {
                let sub_domain = if i == 0 {
                    String::new()
                } else {
                    parts[..i].join(".")
                };
                let service_id = self.get_service_id(&h)?;
                return Ok((sub_domain, service_id));
            }
        }
        Err(Error::Provider(format!("Active24: cannot find zone for {fulldomain}")))
    }

    fn get_service_id(&self, zone: &str) -> Result<String, Error> {
        let endpoint = format!("/v1/user/self/zone/{zone}");
        let resp = self.rest_get(&endpoint)?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!(
                "Active24 zone lookup: HTTP {} {}", resp.status, resp.body
            )));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Provider(format!("Active24 zone response: {e}")))?;
        let id = v.get("id")
            .and_then(|id| {
                id.as_i64().map(|n| n.to_string())
                    .or_else(|| id.as_str().map(|s| s.to_string()))
            })
            .ok_or_else(|| Error::Provider(format!(
                "Active24: no id in zone response: {}", resp.body
            )))?;
        Ok(id)
    }
}

impl DnsProvider for Active24 {
    fn slug() -> &'static str {
        "active24"
    }

    fn env_vars() -> &'static [&'static str] {
        &["Active24_ApiKey", "Active24_ApiSecret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("Active24_ApiKey")
            .ok_or_else(|| Error::Config("Active24_ApiKey required".into()))?
            .clone();
        let api_secret = env.get("Active24_ApiSecret")
            .ok_or_else(|| Error::Config("Active24_ApiSecret required".into()))?
            .clone();
        Ok(Box::new(Active24 { api_key, api_secret }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (sub_domain, service_id) = self.resolve_zone(name)?;
        let endpoint = format!("/v2/service/{service_id}/dns/record");
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "name": sub_domain,
            "content": value,
            "ttl": 300,
        })).unwrap();
        let resp = self.rest_post(&endpoint, &body)?;
        if resp.status >= 400 || resp.body.contains("error") {
            return Err(Error::Provider(format!(
                "Active24 add TXT: HTTP {} {}", resp.status, resp.body
            )));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (sub_domain, service_id) = self.resolve_zone(name)?;
        let endpoint = format!("/v2/service/{service_id}/dns/record");
        let resp = match self.rest_get(&endpoint) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if resp.body.contains("error") {
            return Ok(());
        }

        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let records: Vec<&Value> = v.as_array()
            .map(|a| a.iter().collect())
            .or_else(|| v.get("records").and_then(|r| r.as_array()).map(|a| a.iter().collect()))
            .or_else(|| v.get("data").and_then(|r| r.as_array()).map(|a| a.iter().collect()))
            .unwrap_or_default();

        for record in records {
            let rtype = record.get("type").and_then(|t| t.as_str()).unwrap_or("");
            let rname = record.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let rcontent = record.get("content").and_then(|c| c.as_str()).unwrap_or("");
            if rtype == "TXT" && rname == sub_domain && rcontent == value {
                if let Some(id) = record.get("id").and_then(|i| {
                    i.as_i64().map(|n| n.to_string())
                        .or_else(|| i.as_str().map(|s| s.to_string()))
                }) {
                    let del_endpoint = format!("/v2/service/{service_id}/dns/record/{id}");
                    let _ = self.rest_delete(&del_endpoint);
                }
            }
        }
        Ok(())
    }
}
