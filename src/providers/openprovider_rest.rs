use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct OpenproviderRest {
    username: String,
    password: String,
}

impl DnsProvider for OpenproviderRest {
    fn slug() -> &'static str {
        "openprovider_rest"
    }

    fn env_vars() -> &'static [&'static str] {
        &["OPENPROVIDER_REST_USERNAME", "OPENPROVIDER_REST_PASSWORD"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("OPENPROVIDER_REST_USERNAME")
            .ok_or_else(|| Error::Config("OPENPROVIDER_REST_USERNAME required".into()))?
            .clone();
        let password = env.get("OPENPROVIDER_REST_PASSWORD")
            .ok_or_else(|| Error::Config("OPENPROVIDER_REST_PASSWORD required".into()))?
            .clone();
        Ok(Box::new(OpenproviderRest { username, password }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = self.get_token()?;
        let headers: &[(&str, &str)] = &[("Authorization", &token), ("Accept", "application/json")];
        let (domain_id, domain_name, sub_domain) = self.get_dns_zone(name, &token)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "id": domain_id,
            "name": domain_name,
            "records": {
                "add": [{
                    "name": sub_domain,
                    "ttl": 900,
                    "type": "TXT",
                    "value": value,
                }]
            }
        })).unwrap();
        let url = format!("https://api.openprovider.eu/v1beta/dns/zones/{domain_name}");
        let resp = http::put(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("OpenProvider add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("OpenProvider add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        if !resp.body.contains("\"success\":true") && !resp.body.contains("\"Duplicate record\"") {
            return Err(Error::Provider(format!("OpenProvider add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = match self.get_token() {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };
        let headers: &[(&str, &str)] = &[("Authorization", &token), ("Accept", "application/json")];
        let (domain_id, domain_name, sub_domain) = match self.get_dns_zone(name, &token) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let quoted_value = format!("\"{}\"", value);
        let body = serde_json::to_vec(&serde_json::json!({
            "id": domain_id,
            "name": domain_name,
            "records": {
                "remove": [{
                    "name": sub_domain,
                    "ttl": 900,
                    "type": "TXT",
                    "value": quoted_value,
                }]
            }
        })).unwrap();
        let url = format!("https://api.openprovider.eu/v1beta/dns/zones/{domain_name}");
        let resp = match http::put(&url, &body, "application/json", headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if !resp.body.contains("\"success\":true") {
            eprintln!("warning: OpenProvider remove TXT: {}", resp.body);
        }
        Ok(())
    }
}

impl OpenproviderRest {
    fn get_token(&self) -> Result<String, Error> {
        let body = serde_json::to_vec(&serde_json::json!({
            "ip": "0.0.0.0",
            "password": self.password,
            "username": self.username,
        })).unwrap();
        let resp = http::post("https://api.openprovider.eu/v1beta/auth/login", &body, "application/json", &[("Accept", "application/json")])
            .map_err(|e| Error::Provider(format!("OpenProvider auth: {e}")))?;
        let token = extract_token(&resp.body)
            .ok_or_else(|| Error::Provider(format!("OpenProvider auth: no token in response: {}", resp.body)))?;
        Ok(format!("Bearer {token}"))
    }

    fn get_dns_zone(&self, domain: &str, token: &str) -> Result<(Value, String, String), Error> {
        let headers: &[(&str, &str)] = &[("Authorization", token), ("Accept", "application/json")];
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                continue;
            }
            let url = format!("https://api.openprovider.eu/v1beta/dns/zones/{h}");
            let resp = http::get(&url, headers)
                .map_err(|e| Error::Provider(format!("OpenProvider zone lookup: {e}")))?;
            if resp.body.contains(&format!("\"name\":\"{}\"", h)) {
                let v: Value = serde_json::from_str(&resp.body)
                    .map_err(|e| Error::Json(format!("OpenProvider zone lookup: {e}")))?;
                let id = v.pointer("/data/0/id")
                    .or_else(|| v.get("data").and_then(|d| d.get("id")))
                    .cloned()
                    .unwrap_or(Value::Null);
                let sub_domain = parts[..i].join(".");
                return Ok((id, h, sub_domain));
            }
        }
        Err(Error::Provider(format!("OpenProvider: DNS zone not found for {}", domain)))
    }
}

fn extract_token(body: &str) -> Option<String> {
    let needle = "\"token\"";
    let pos = body.find(needle)?;
    let rest = &body[pos + needle.len()..];
    let colon_pos = rest.find(':')?;
    let rest = &rest[colon_pos + 1..];
    let quote_start = rest.find('"')?;
    let rest = &rest[quote_start + 1..];
    let quote_end = rest.find('"')?;
    Some(rest[..quote_end].to_string())
}
