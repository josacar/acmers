use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Technitium {
    server: String,
    token: String,
}

impl DnsProvider for Technitium {
    fn slug() -> &'static str {
        "technitium"
    }

    fn env_vars() -> &'static [&'static str] {
        &["TECHNITIUM_Server", "TECHNITIUM_Token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let server = env.get("TECHNITIUM_Server")
            .ok_or_else(|| Error::Config("TECHNITIUM_Server required".into()))?
            .trim_end_matches('/')
            .to_string();
        let token = env.get("TECHNITIUM_Token")
            .ok_or_else(|| Error::Config("TECHNITIUM_Token required".into()))?
            .clone();
        Ok(Box::new(Technitium { server, token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let url = format!(
            "{}/api/zones/records/add?token={}&domain={}&type=TXT&name={}&value={}&ttl=120",
            self.server,
            urlencode(&self.token),
            urlencode(domain),
            urlencode(name),
            urlencode(value),
        );
        let resp = http::post(&url, b"", "application/x-www-form-urlencoded", &[])
            .map_err(|e| Error::Provider(format!("Technitium add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Technitium add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Technitium response: {e}")))?;
        if v.get("status").and_then(|s| s.as_str()) == Some("error") {
            let msg = v.get("errorMessage").and_then(|m| m.as_str()).unwrap_or("unknown");
            return Err(Error::Provider(format!("Technitium add TXT: {msg}")));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let url = format!(
            "{}/api/zones/records/delete?token={}&domain={}&type=TXT&name={}&value={}",
            self.server,
            urlencode(&self.token),
            urlencode(domain),
            urlencode(name),
            urlencode(value),
        );
        let _ = http::post(&url, b"", "application/x-www-form-urlencoded", &[]);
        Ok(())
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
