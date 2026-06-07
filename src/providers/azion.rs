use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Azion {
    token: String,
}

impl DnsProvider for Azion {
    fn slug() -> &'static str {
        "azion"
    }

    fn env_vars() -> &'static [&'static str] {
        &["AZION_Email", "AZION_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let email = env.get("AZION_Email")
            .ok_or_else(|| Error::Config("AZION_Email required".into()))?.clone();
        let password = env.get("AZION_Password")
            .ok_or_else(|| Error::Config("AZION_Password required".into()))?.clone();

        let login_body = serde_json::json!({"email": email, "password": password});
        let resp = http::post(
            "https://api.azionapi.net/tokens",
            &serde_json::to_vec(&login_body).unwrap(),
            "application/json",
            &[]
        ).map_err(|e| Error::Provider(format!("azion login: {e}")))?;

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("azion login response: {e}")))?;
        let token = v.get("token").and_then(|t| t.as_str())
            .ok_or_else(|| Error::Provider(format!("azion login: no token in response")))?;

        Ok(Box::new(Azion { token: token.to_string() }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let url = format!("https://api.azionapi.net/intelligent_dns/{zone_id}/records");
        let body = serde_json::json!({
            "record_type": "TXT",
            "entry": name,
            "answers_list": [value],
            "ttl": 120,
        });
        let auth = format!("Bearer {}", self.token);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("azion add TXT: {e}")))?;
        if resp.status >= 300 {
            return Err(Error::Provider(format!("azion add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = match self.resolve_zone(domain) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let auth = format!("Bearer {}", self.token);
        let url = format!("https://api.azionapi.net/intelligent_dns/{zone_id}/records");
        let resp = match http::get(&url, &[("Authorization", &auth)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records = v.get("results").and_then(|r| r.as_array())
            .or_else(|| v.as_array());
        if let Some(records) = records {
            for record in records {
                if record.get("record_type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("entry").and_then(|n| n.as_str()) == Some(name)
                {
                    if let Some(answers) = record.get("answers_list").and_then(|a| a.as_array()) {
                        if answers.iter().any(|a| a.as_str() == Some(value)) {
                            if let Some(id) = record.get("id").and_then(|i| {
                                if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                            }) {
                                let del_url = format!("https://api.azionapi.net/intelligent_dns/{zone_id}/records/{id}");
                                let _ = http::delete(&del_url, &[("Authorization", &auth)]);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl Azion {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let auth = format!("Bearer {}", self.token);
        let resp = http::get("https://api.azionapi.net/intelligent_dns", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("azion list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("azion zones: {e}")))?;
        let zones = v.get("results").and_then(|r| r.as_array())
            .or_else(|| v.as_array());
        if let Some(zones) = zones {
            for zone in zones {
                if let Some(nm) = zone.get("domain_name").and_then(|n| n.as_str())
                    .or_else(|| zone.get("name").and_then(|n| n.as_str()))
                {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        if let Some(id) = zone.get("id").and_then(|i| {
                            if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                        }) {
                            return Ok(id);
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
