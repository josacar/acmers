use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const STD_B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub struct Namecom {
    auth_header: String,
}

impl DnsProvider for Namecom {
    fn slug() -> &'static str {
        "namecom"
    }

    fn env_vars() -> &'static [&'static str] {
        &["Namecom_Username", "Namecom_Token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("Namecom_Username")
            .ok_or_else(|| Error::Config("Namecom_Username required".into()))?
            .clone();
        let token = env.get("Namecom_Token")
            .ok_or_else(|| Error::Config("Namecom_Token required".into()))?
            .clone();
        let raw = format!("{username}:{token}");
        let encoded = std_b64(raw.as_bytes());
        let auth_header = format!("Basic {encoded}");
        Ok(Box::new(Namecom { auth_header }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.auth_header)];
        let domain_name = self.resolve_zone(domain, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "host": name,
            "answer": value,
            "ttl": 120,
        })).unwrap();
        let url = format!("https://api.name.com/v4/domains/{domain_name}/records");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("Name.com add TXT: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Name.com response: {e}")))?;
        if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
            if resp.status >= 400 {
                return Err(Error::Provider(format!("Name.com add TXT: {msg}")));
            }
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.auth_header)];
        let domain_name = match self.resolve_zone(domain, headers) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };
        let list_url = format!("https://api.name.com/v4/domains/{domain_name}/records?perPage=1000");
        let resp = match http::get(&list_url, headers) {
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
                    && record.get("host").and_then(|h| h.as_str()) == Some(name)
                    && record.get("answer").and_then(|a| a.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| i.as_i64()) {
                        let del_url = format!("https://api.name.com/v4/domains/{domain_name}/records/{id}");
                        let _ = http::delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Namecom {
    fn resolve_zone(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let resp = http::get("https://api.name.com/v4/domains", headers)
            .map_err(|e| Error::Provider(format!("Name.com list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Name.com domains: {e}")))?;
        if let Some(domains) = v.get("domains").and_then(|d| d.as_array()) {
            for d in domains {
                if let Some(name) = d.get("domainName").and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        return Ok(name.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}

fn std_b64(input: &[u8]) -> String {
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(STD_B64[((triple >> 18) & 0x3f) as usize] as char);
        out.push(STD_B64[((triple >> 12) & 0x3f) as usize] as char);
        out.push(if chunk.len() > 1 { STD_B64[((triple >> 6) & 0x3f) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { STD_B64[(triple & 0x3f) as usize] as char } else { '=' });
    }
    out
}
