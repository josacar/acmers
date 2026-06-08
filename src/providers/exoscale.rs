use std::collections::HashMap;
use ring::hmac;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api-ch-gva-2.exoscale.com/v2";

pub struct Exoscale {
    api_key: String,
    api_secret: String,
}

impl DnsProvider for Exoscale {
    fn slug() -> &'static str {
        "exoscale"
    }

    fn env_vars() -> &'static [&'static str] {
        &["EXOSCALE_API_KEY", "EXOSCALE_SECRET_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("EXOSCALE_API_KEY")
            .ok_or_else(|| Error::Config("EXOSCALE_API_KEY required".into()))?
            .clone();
        let api_secret = env.get("EXOSCALE_SECRET_KEY")
            .ok_or_else(|| Error::Config("EXOSCALE_SECRET_KEY required".into()))?
            .clone();
        Ok(Box::new(Exoscale { api_key, api_secret }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone_id, sub_domain) = self.resolve_zone(name)?;
        let record_name = if sub_domain.is_empty() {
            "_acme-challenge".to_string()
        } else {
            format!("_acme-challenge.{sub_domain}")
        };
        let body = serde_json::json!({
            "name": record_name,
            "type": "TXT",
            "content": value,
            "ttl": 120,
        });
        let body_str = serde_json::to_string(&body).unwrap();
        let path = format!("/dns-domain/{zone_id}/record");
        let resp = self.request("POST", &path, Some(&body_str))
            .map_err(|e| Error::Provider(format!("exoscale add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("exoscale add TXT: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone_id, sub_domain) = match self.resolve_zone(name) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let record_name = if sub_domain.is_empty() {
            "_acme-challenge".to_string()
        } else {
            format!("_acme-challenge.{sub_domain}")
        };
        let path = format!("/dns-domain/{zone_id}/record");
        let resp = match self.request("GET", &path, None) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(records) = v.as_array() {
            for rec in records {
                let rec_type = rec.get("type").and_then(|t| t.as_str());
                let rec_name = rec.get("name").and_then(|n| n.as_str());
                let rec_content = rec.get("content").and_then(|c| c.as_str());
                if rec_type == Some("TXT") && rec_name == Some(record_name.as_str()) && rec_content == Some(value) {
                    if let Some(id) = rec.get("id").and_then(|i| i.as_str()) {
                        let del_path = format!("/dns-domain/{zone_id}/record/{id}");
                        self.request("DELETE", &del_path, None).ok();
                    }
                }
            }
        }
        Ok(())
    }
}

impl Exoscale {
    fn sign(&self, method: &str, path: &str, body: Option<&str>) -> String {
        let expiration = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 300;
        let expiration_str = expiration.to_string();
        let body_str = body.unwrap_or("");
        let message = format!("{method} /v2{path}\n{body_str}\n\n\n{expiration_str}");
        let key = hmac::Key::new(hmac::HMAC_SHA256, self.api_secret.as_bytes());
        let tag = hmac::sign(&key, message.as_bytes());
        let signature = base64::encode_std(tag.as_ref());
        format!(
            "EXO2-HMAC-SHA256 credential={},expires={},signature={}",
            self.api_key, expiration_str, signature
        )
    }

    fn request(&self, method: &str, path: &str, body: Option<&str>) -> Result<http::Response, Error> {
        let url = format!("{BASE_URL}{path}");
        let auth = self.sign(method, path, body);
        let headers = [
            ("Authorization", auth.as_str()),
            ("Accept", "application/json"),
        ];
        match method {
            "GET" => http::get(&url, &headers)
                .map_err(|e| Error::Provider(format!("exoscale GET {path}: {e}"))),
            "POST" => {
                let body_bytes = body.unwrap_or("").as_bytes();
                http::post(&url, body_bytes, "application/json", &headers)
                    .map_err(|e| Error::Provider(format!("exoscale POST {path}: {e}")))
            }
            "DELETE" => http::delete(&url, &headers)
                .map_err(|e| Error::Provider(format!("exoscale DELETE {path}: {e}"))),
            _ => Err(Error::Provider(format!("exoscale unsupported method: {method}"))),
        }
    }

    fn resolve_zone(&self, fulldomain: &str) -> Result<(String, String), Error> {
        let domain = fulldomain.to_lowercase();
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 2..parts.len() {
            let candidate = parts[i..].join(".");
            if candidate.is_empty() {
                break;
            }
            let resp = match self.request("GET", "/dns-domain", None) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let v: Value = match serde_json::from_str(&resp.body) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if let Some(domains) = v.get("dns-domains").and_then(|d| d.as_array()) {
                for d in domains {
                    let dname = d.get("unicode-name").and_then(|n| n.as_str());
                    if dname == Some(candidate.as_str()) {
                        if let Some(id) = d.get("id").and_then(|i| i.as_str()) {
                            let sub = parts[..i].join(".");
                            let sub = sub.strip_prefix("_acme-challenge.")
                                .unwrap_or(if sub == "_acme-challenge" { "" } else { &sub });
                            return Ok((id.to_string(), sub.to_string()));
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("exoscale zone not found for {fulldomain}")))
    }
}
