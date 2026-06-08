use std::collections::HashMap;

use ring::hmac;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.dns.constellix.com/v1";

pub struct Constellix {
    api_key: String,
    api_secret: String,
}

impl DnsProvider for Constellix {
    fn slug() -> &'static str {
        "constellix"
    }

    fn env_vars() -> &'static [&'static str] {
        &["CONSTELLIX_Key", "CONSTELLIX_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("CONSTELLIX_Key")
            .ok_or_else(|| Error::Config("CONSTELLIX_Key required".into()))?.clone();
        let api_secret = env.get("CONSTELLIX_Secret")
            .ok_or_else(|| Error::Config("CONSTELLIX_Secret required".into()))?.clone();
        Ok(Box::new(Constellix { api_key, api_secret }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (domain_id, sub_domain) = self.resolve_zone(name)?;
        std::thread::sleep(std::time::Duration::from_secs(2));
        let body = serde_json::json!([{
            "type": "txt",
            "add": true,
            "set": {
                "name": sub_domain,
                "ttl": 60,
                "roundRobin": [{"value": value}]
            }
        }]);
        let url = format!("{BASE_URL}/domains/{domain_id}/records");
        let headers = self.auth_headers();
        let header_refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &header_refs)
            .map_err(|e| Error::Provider(format!("constellix add TXT: {e}")))?;
        if resp.status >= 300 {
            return Err(Error::Provider(format!("constellix add TXT: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, _value: &str) -> ProviderResult {
        let (domain_id, sub_domain) = match self.resolve_zone(name) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        std::thread::sleep(std::time::Duration::from_secs(2));
        let body = serde_json::json!([{
            "type": "txt",
            "delete": true,
            "filter": {
                "field": "name",
                "op": "eq",
                "value": sub_domain
            }
        }]);
        let url = format!("{BASE_URL}/domains/{domain_id}/records");
        let headers = self.auth_headers();
        let header_refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let _ = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &header_refs);
        Ok(())
    }
}

impl Constellix {
    fn auth_headers(&self) -> Vec<(String, String)> {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .to_string();
        let signing_key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, self.api_secret.as_bytes());
        let tag = hmac::sign(&signing_key, ts.as_bytes());
        let hmac_b64 = base64::encode_std(tag.as_ref());
        vec![
            ("x-cnsdns-apiKey".to_string(), self.api_key.clone()),
            ("x-cnsdns-requestDate".to_string(), ts),
            ("x-cnsdns-hmac".to_string(), hmac_b64),
        ]
    }

    fn rest_get(&self, endpoint: &str) -> Result<http::Response, Error> {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let url = format!("{BASE_URL}/{endpoint}");
        let headers = self.auth_headers();
        let header_refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        http::get(&url, &header_refs)
            .map_err(|e| Error::Provider(format!("constellix GET {endpoint}: {e}")))
    }

    fn resolve_zone(&self, fulldomain: &str) -> Result<(String, String), Error> {
        let domain = fulldomain.to_lowercase();
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 2..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                break;
            }
            let resp = match self.rest_get(&format!("domains/search?exact={h}")) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let v: Value = match serde_json::from_str(&resp.body) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if v.get("name").and_then(|n| n.as_str()) == Some(h.as_str()) {
                if let Some(id) = v.get("id").and_then(|id| id.as_i64()).map(|id| id.to_string())
                    .or_else(|| v.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
                {
                    let sub_domain = parts[..i].join(".");
                    return Ok((id, sub_domain));
                }
            }
        }
        Err(Error::Provider(format!("constellix zone not found for {fulldomain}")))
    }
}
