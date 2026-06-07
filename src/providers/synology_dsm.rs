use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct SynologyDsm {
    base: String,
    sid: String,
}

impl DnsProvider for SynologyDsm {
    fn slug() -> &'static str {
        "synology_dsm"
    }

    fn env_vars() -> &'static [&'static str] {
        &["SYNOLOGY_DSM_HOSTNAME", "SYNOLOGY_DSM_USERNAME", "SYNOLOGY_DSM_PASSWORD"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let hostname = env.get("SYNOLOGY_DSM_HOSTNAME")
            .ok_or_else(|| Error::Config("SYNOLOGY_DSM_HOSTNAME required".into()))?
            .clone();
        let username = env.get("SYNOLOGY_DSM_USERNAME")
            .ok_or_else(|| Error::Config("SYNOLOGY_DSM_USERNAME required".into()))?
            .clone();
        let password = env.get("SYNOLOGY_DSM_PASSWORD")
            .ok_or_else(|| Error::Config("SYNOLOGY_DSM_PASSWORD required".into()))?
            .clone();

        let base = build_base_url(&hostname);
        let sid = login(&base, &username, &password)?;

        Ok(Box::new(SynologyDsm { base, sid }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = self.resolve_zone(domain)?;
        let url = format!(
            "{}?api=SYNO.DNS.Server&version=1&method=add&_sid={}&zone={}&name={}&type=TXT&content={}&ttl=120",
            self.base,
            url_encode(&self.sid),
            url_encode(&zone),
            url_encode(name),
            url_encode(value),
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("synology add record: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("synology add response: {e}")))?;
        if !v.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
            let err = v.get("error").and_then(|e| e.get("code")).and_then(|c| c.as_i64()).unwrap_or(0);
            return Err(Error::Provider(format!("synology add record failed: error={err}, body={}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = match self.resolve_zone(domain) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let records = match self.list_records(&zone) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        for rec in &records {
            let rec_name = rec.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let rec_type = rec.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let rec_content = rec.get("content").and_then(|v| v.as_str()).unwrap_or("");
            if rec_type == "TXT" && rec_name == name && rec_content == value {
                let rec_id: Option<String> = rec.get("id").and_then(|v| v.as_str()).map(|s| s.to_string())
                    .or_else(|| rec.get("id").and_then(|v| v.as_i64()).map(|n| n.to_string()));
                if let Some(ref rec_id) = rec_id {
                    let url = format!(
                        "{}?api=SYNO.DNS.Server&version=1&method=delete&_sid={}&zone={}&record_id={}",
                        self.base,
                        url_encode(&self.sid),
                        url_encode(&zone),
                        url_encode(rec_id),
                    );
                    let _ = http::get(&url, &[]);
                }
            }
        }
        Ok(())
    }
}

impl SynologyDsm {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let url = format!(
            "{}?api=SYNO.DNS.Server&version=1&method=list&_sid={}",
            self.base,
            url_encode(&self.sid),
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("synology list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("synology zones response: {e}")))?;
        if let Some(zones) = v.get("data").and_then(|d| d.get("zones")).and_then(|z| z.as_array()) {
            let parts: Vec<&str> = domain.split('.').collect();
            for i in 0..parts.len().saturating_sub(1) {
                let candidate = parts[i..].join(".");
                for z in zones {
                    if let Some(name) = z.get("name").and_then(|n| n.as_str()) {
                        if name == candidate {
                            return Ok(name.to_string());
                        }
                    }
                }
            }
            for z in zones {
                if let Some(name) = z.get("name").and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        return Ok(name.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("synology: zone not found for {domain}")))
    }

    fn list_records(&self, zone: &str) -> Result<Vec<Value>, Error> {
        let url = format!(
            "{}?api=SYNO.DNS.Server&version=1&method=list&_sid={}&zone={}",
            self.base,
            url_encode(&self.sid),
            url_encode(zone),
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("synology list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("synology records response: {e}")))?;
        if let Some(records) = v.get("data").and_then(|d| d.get("records")).and_then(|r| r.as_array()) {
            return Ok(records.clone());
        }
        Ok(Vec::new())
    }
}

fn build_base_url(host: &str) -> String {
    let host = host.trim();
    if host.contains("://") {
        let trimmed = host.trim_end_matches('/');
        if trimmed.ends_with("/webapi/entry.cgi") || trimmed.ends_with("/webapi/entry.cgi/") {
            trimmed.trim_end_matches('/').to_string()
        } else if trimmed.ends_with("/webapi") {
            format!("{trimmed}/entry.cgi")
        } else {
            format!("{trimmed}/webapi/entry.cgi")
        }
    } else {
        format!("https://{host}:5001/webapi/entry.cgi")
    }
}

fn login(base: &str, username: &str, password: &str) -> Result<String, Error> {
    let url = format!(
        "{}?api=SYNO.API.Auth&version=6&method=login&account={}&passwd={}&format=sid",
        base,
        url_encode(username),
        url_encode(password),
    );
    let resp = http::get(&url, &[])
        .map_err(|e| Error::Provider(format!("synology login: {e}")))?;
    let v: Value = serde_json::from_str(&resp.body)
        .map_err(|e| Error::Json(format!("synology login response: {e}")))?;
    if !v.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
        let code = v.get("error").and_then(|e| e.get("code")).and_then(|c| c.as_i64()).unwrap_or(0);
        if code == 404 {
            return Err(Error::Provider("synology login: 2FA required, use a non-2FA account or set SYNOLOGY_DSM_DEVICE_ID/SYNOLOGY_DSM_DEVICE_NAME".into()));
        }
        return Err(Error::Provider(format!("synology login failed: error={code}, body={}", resp.body)));
    }
    v.get("data").and_then(|d| d.get("sid")).and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| Error::Provider("synology login: no sid in response".into()))
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => {
                out.push('%');
                out.push(hex_digit((b >> 4) & 0xf));
                out.push(hex_digit(b & 0xf));
            }
        }
    }
    out
}

fn hex_digit(n: u8) -> char {
    if n < 10 { (b'0' + n) as char } else { (b'A' + n - 10) as char }
}
