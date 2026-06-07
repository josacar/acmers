use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Hestiacp {
    base: String,
    username: String,
    session: String,
}

impl DnsProvider for Hestiacp {
    fn slug() -> &'static str {
        "hestiacp"
    }

    fn env_vars() -> &'static [&'static str] {
        &["HESTIACP_USERNAME", "HESTIACP_PASSWORD", "HESTIACP_HOST"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let host = env.get("HESTIACP_HOST")
            .ok_or_else(|| Error::Config("HESTIACP_HOST required".into()))?
            .clone();
        let username = env.get("HESTIACP_USERNAME")
            .ok_or_else(|| Error::Config("HESTIACP_USERNAME required".into()))?
            .clone();
        let password = env.get("HESTIACP_PASSWORD")
            .ok_or_else(|| Error::Config("HESTIACP_PASSWORD required".into()))?
            .clone();

        let base = build_base_url(&host);
        let login_body = format!(
            "user={}&password={}&returncode=yes",
            url_encode(&username),
            url_encode(&password),
        );
        let resp = http::post(&base, login_body.as_bytes(), "application/x-www-form-urlencoded", &[])
            .map_err(|e| Error::Provider(format!("hestiacp login: {e}")))?;

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("hestiacp login response: {e}")))?;

        if v.get("login").and_then(|l| l.as_str()) != Some("success") {
            return Err(Error::Provider(format!("hestiacp login failed: {}", resp.body)));
        }

        let session = v.get("answer").and_then(|a| a.as_str())
            .ok_or_else(|| Error::Provider("hestiacp: no session key in login response".into()))?
            .to_string();

        Ok(Box::new(Hestiacp { base, username, session }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let url = format!(
            "{}?hash={}&cmd=v-add-dns-record&arg={}&arg={}&arg={}&arg=TXT&arg={}&arg=120&returncode=yes",
            self.base, self.session, self.username, domain, name, value,
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("hestiacp add TXT: {e}")))?;
        if resp.status >= 400 || (resp.body.contains("Error") && !resp.body.starts_with("{\"")) {
            return Err(Error::Provider(format!("hestiacp add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let records = match self.list_records(domain) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if let Some((record_id, _)) = records.iter().find(|(_, r)| {
            r.get("RECORD").and_then(|v| v.as_str()) == Some(name)
                && r.get("TYPE").and_then(|v| v.as_str()) == Some("TXT")
                && r.get("VALUE").and_then(|v| v.as_str()) == Some(value)
        }) {
            let url = format!(
                "{}?hash={}&cmd=v-delete-dns-record&arg={}&arg={}&arg={}&returncode=yes",
                self.base, self.session, self.username, domain, record_id,
            );
            let _ = http::get(&url, &[]);
        }
        Ok(())
    }
}

impl Hestiacp {
    fn list_records(&self, domain: &str) -> Result<Vec<(String, Value)>, Error> {
        let url = format!(
            "{}?hash={}&cmd=v-list-dns-records&arg={}&arg={}&returncode=yes",
            self.base, self.session, self.username, domain,
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("hestiacp list records: {e}")))?;

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("hestiacp list records: {e}")))?;

        let mut records = Vec::new();
        if let Some(obj) = v.as_object() {
            for (key, val) in obj {
                if val.is_object() && key.parse::<u64>().is_ok() {
                    records.push((key.clone(), val.clone()));
                }
            }
        }
        Ok(records)
    }
}

fn build_base_url(host: &str) -> String {
    if host.contains("://") {
        let trimmed = host.trim_end_matches('/');
        if trimmed.ends_with("/api") || trimmed.ends_with("/api/") {
            format!("{trimmed}/")
        } else {
            format!("{trimmed}/api/")
        }
    } else if host.contains(':') {
        format!("https://{host}/api/")
    } else {
        format!("https://{host}:8083/api/")
    }
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => {
                out.push('%');
                out.push(to_hex((b >> 4) & 0xf));
                out.push(to_hex(b & 0xf));
            }
        }
    }
    out
}

fn to_hex(n: u8) -> char {
    if n < 10 { (b'0' + n) as char } else { (b'A' + n - 10) as char }
}
