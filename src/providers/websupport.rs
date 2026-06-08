use std::collections::HashMap;
use serde_json::Value;
use ring::hmac;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const API_BASE: &str = "https://rest.websupport.sk";

pub struct Websupport {
    api_key: String,
    api_secret: String,
}

impl Websupport {
    fn auth_headers(&self, method: &str, path: &str) -> Vec<(String, String)> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let now = time::OffsetDateTime::now_utc();
        let datez = format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+0000",
            now.year(), now.month() as u8, now.day(),
            now.hour(), now.minute(), now.second()
        );

        let canonical = format!("{method} {path} {timestamp}");

        let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, self.api_secret.as_bytes());
        let tag = hmac::sign(&key, canonical.as_bytes());
        let signature = crate::base64::hex(tag.as_ref());

        let auth_plain = format!("{}:{}", self.api_key, signature);
        let auth_b64 = crate::base64::encode_std(auth_plain.as_bytes());

        vec![
            ("Accept".to_string(), "application/json".to_string()),
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Authorization".to_string(), format!("Basic {auth_b64}")),
            ("Date".to_string(), datez),
        ]
    }

    fn header_refs(headers: &[(String, String)]) -> Vec<(&str, &str)> {
        headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
    }

    fn rest_get(&self, path: &str) -> Result<http::Response, Error> {
        let headers = self.auth_headers("GET", path);
        let hrefs = Self::header_refs(&headers);
        let url = format!("{API_BASE}{path}");
        http::get(&url, &hrefs)
            .map_err(|e| Error::Provider(format!("Websupport GET {path}: {e}")))
    }

    fn rest_post(&self, path: &str, body: &[u8]) -> Result<http::Response, Error> {
        let headers = self.auth_headers("POST", path);
        let hrefs = Self::header_refs(&headers);
        let url = format!("{API_BASE}{path}");
        http::post(&url, body, "application/json", &hrefs)
            .map_err(|e| Error::Provider(format!("Websupport POST {path}: {e}")))
    }

    fn rest_delete(&self, path: &str) -> Result<http::Response, Error> {
        let headers = self.auth_headers("DELETE", path);
        let hrefs = Self::header_refs(&headers);
        let url = format!("{API_BASE}{path}");
        http::delete(&url, &hrefs)
            .map_err(|e| Error::Provider(format!("Websupport DELETE {path}: {e}")))
    }

    fn resolve_zone(&self, fulldomain: &str) -> Result<(String, String), Error> {
        let resp = self.rest_get("/v1/user/self/zone")?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!(
                "Websupport zone list: HTTP {} {}", resp.status, resp.body
            )));
        }

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Websupport zone list: {e}")))?;

        let zones: Vec<&str> = v.get("items").and_then(|i| i.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item.get("name").and_then(|n| n.as_str()))
                    .collect()
            })
            .unwrap_or_default();

        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                break;
            }
            if zones.contains(&h.as_str()) {
                let sub_domain = if i == 0 {
                    String::new()
                } else {
                    parts[..i].join(".")
                };
                return Ok((sub_domain, h));
            }
        }
        Err(Error::Provider(format!("Websupport: cannot find zone for {fulldomain}")))
    }
}

impl DnsProvider for Websupport {
    fn slug() -> &'static str {
        "websupport"
    }

    fn env_vars() -> &'static [&'static str] {
        &["WS_ApiKey", "WS_ApiSecret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("WS_ApiKey")
            .ok_or_else(|| Error::Config("WS_ApiKey required".into()))?
            .clone();
        let api_secret = env.get("WS_ApiSecret")
            .ok_or_else(|| Error::Config("WS_ApiSecret required".into()))?
            .clone();
        Ok(Box::new(Websupport { api_key, api_secret }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (sub_domain, zone) = self.resolve_zone(name)?;
        let path = format!("/v1/user/self/zone/{zone}/record");
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "name": sub_domain,
            "content": value,
            "ttl": 120,
        })).unwrap();
        let resp = self.rest_post(&path, &body)?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!(
                "Websupport add TXT: HTTP {} {}", resp.status, resp.body
            )));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (sub_domain, zone) = match self.resolve_zone(name) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let path = format!("/v1/user/self/zone/{zone}/record");
        let resp = match self.rest_get(&path) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records: Option<&Vec<Value>> = v.get("items").and_then(|i| i.as_array());
        if let Some(arr) = records {
            for record in arr {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(sub_domain.as_str())
                    && record.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_path = format!("/v1/user/self/zone/{zone}/record/{id}");
                        let _ = self.rest_delete(&del_path);
                    }
                }
            }
        }
        Ok(())
    }
}
