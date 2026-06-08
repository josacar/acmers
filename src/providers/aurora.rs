use std::collections::HashMap;

use ring::hmac;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.auroradns.eu";

pub struct Aurora {
    api_key: String,
    api_secret: String,
}

impl DnsProvider for Aurora {
    fn slug() -> &'static str { "aurora" }
    fn env_vars() -> &'static [&'static str] { &["AURORA_Key", "AURORA_Secret"] }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("AURORA_Key")
            .ok_or_else(|| Error::Config("AURORA_Key required".into()))?.clone();
        let api_secret = env.get("AURORA_Secret")
            .ok_or_else(|| Error::Config("AURORA_Secret required".into()))?.clone();
        Ok(Box::new(Aurora { api_key, api_secret }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone_id, sub_domain) = self.resolve_zone(name)?;
        let body = serde_json::json!({
            "type": "TXT",
            "name": sub_domain,
            "content": value,
            "ttl": 300,
        });
        let endpoint = format!("zones/{zone_id}/records");
        let url = format!("{BASE_URL}/{endpoint}");
        let (authorization, timestamp) = self.sign("POST", &endpoint);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[
            ("Authorization", &authorization),
            ("X-AuroraDNS-Date", &timestamp),
        ]).map_err(|e| Error::Provider(format!("aurora add TXT: {e}")))?;
        if resp.body.contains(value) || resp.body.contains("RecordExistsError") {
            return Ok(());
        }
        Err(Error::Provider(format!("aurora add TXT: {} {}", resp.status, resp.body)))
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone_id, sub_domain) = match self.resolve_zone(name) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let endpoint = format!("zones/{zone_id}/records");
        let url = format!("{BASE_URL}/{endpoint}");
        let (authorization, timestamp) = self.sign("GET", &endpoint);
        let resp = match http::get(&url, &[
            ("Authorization", &authorization),
            ("X-AuroraDNS-Date", &timestamp),
        ]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(records) = v.as_array() {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(sub_domain.as_str())
                    && record.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| i.as_str()) {
                        let del_endpoint = format!("zones/{zone_id}/records/{id}");
                        let del_url = format!("{BASE_URL}/{del_endpoint}");
                        let (del_auth, del_ts) = self.sign("DELETE", &del_endpoint);
                        let _ = http::delete(&del_url, &[
                            ("Authorization", &del_auth),
                            ("X-AuroraDNS-Date", &del_ts),
                        ]);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Aurora {
    fn sign(&self, method: &str, endpoint: &str) -> (String, String) {
        let now = time::OffsetDateTime::now_utc();
        let (y, mo, d) = (now.year(), now.month() as u8, now.day());
        let (h, mi, s) = (now.hour(), now.minute(), now.second());
        let timestamp = format!("{y:04}{mo:02}{d:02}T{h:02}{mi:02}{s:02}Z");

        let sign_data = format!("{method}/{endpoint}{timestamp}");
        let signing_key = hmac::Key::new(hmac::HMAC_SHA256, self.api_secret.as_bytes());
        let tag = hmac::sign(&signing_key, sign_data.as_bytes());
        let signature = base64::encode_std(tag.as_ref());

        let creds = format!("{}:{}", self.api_key, signature);
        let authorization = format!("AuroraDNSv1 {}", base64::encode_std(creds.as_bytes()));
        (authorization, timestamp)
    }

    fn resolve_zone(&self, fulldomain: &str) -> Result<(String, String), Error> {
        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            let endpoint = format!("zones/{h}");
            let url = format!("{BASE_URL}/{endpoint}");
            let (authorization, timestamp) = self.sign("GET", &endpoint);
            let resp = match http::get(&url, &[
                ("Authorization", &authorization),
                ("X-AuroraDNS-Date", &timestamp),
            ]) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let v: Value = match serde_json::from_str(&resp.body) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if v.get("name").and_then(|n| n.as_str()) == Some(h.as_str()) {
                if let Some(id) = v.get("id").and_then(|i| i.as_str()) {
                    let sub_domain = parts[..i].join(".");
                    return Ok((id.to_string(), sub_domain));
                }
            }
        }
        Err(Error::Provider(format!("aurora zone not found for {fulldomain}")))
    }
}
