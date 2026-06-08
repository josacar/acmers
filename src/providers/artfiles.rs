use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://dcp.c.artfiles.de/api";

pub struct Artfiles {
    username: String,
    password: String,
}

impl DnsProvider for Artfiles {
    fn slug() -> &'static str {
        "artfiles"
    }

    fn env_vars() -> &'static [&'static str] {
        &["AF_API_USERNAME", "AF_API_PASSWORD"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("AF_API_USERNAME")
            .ok_or_else(|| Error::Config("AF_API_USERNAME required".into()))?
            .clone();
        let password = env.get("AF_API_PASSWORD")
            .ok_or_else(|| Error::Config("AF_API_PASSWORD required".into()))?
            .clone();
        Ok(Box::new(Artfiles { username, password }))
    }

    fn add_txt(&self, domain: &str, _name: &str, value: &str) -> ProviderResult {
        let auth = self.auth_header();
        let zone = self.get_zone(domain, &auth)?;
        let records = self.get_txt_records(&zone, &auth)?;
        let new_records = if records.is_empty() {
            format!("_acme-challenge \"{}\"", value)
        } else {
            format!("{}\n_acme-challenge \"{}\"", records, value)
        };
        self.set_txt_records(&zone, &new_records, &auth)
    }

    fn remove_txt(&self, domain: &str, _name: &str, value: &str) -> ProviderResult {
        let auth = self.auth_header();
        let zone = self.get_zone(domain, &auth)?;
        let records = self.get_txt_records(&zone, &auth)?;
        let target = format!("_acme-challenge \"{}\"", value);
        let filtered: Vec<&str> = records.lines()
            .filter(|line| line.trim() != target)
            .collect();
        let new_records = filtered.join("\n");
        self.set_txt_records(&zone, &new_records, &auth)
    }
}

impl Artfiles {
    fn auth_header(&self) -> String {
        let creds = format!("{}:{}", self.username, self.password);
        format!("Basic {}", base64::encode_std(creds.as_bytes()))
    }

    fn get_zone(&self, fqdn: &str, auth: &str) -> Result<String, Error> {
        let resp = http::get(
            &format!("{}/domain/get_domains.html", BASE_URL),
            &[("Authorization", auth)],
        ).map_err(|e| Error::Provider(format!("artfiles get domains: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("artfiles get domains: HTTP {}", resp.status)));
        }
        let domains = &resp.body;
        let mut current = fqdn;
        loop {
            if domains.contains(current) {
                return Ok(current.to_string());
            }
            if let Some(pos) = current.find('.') {
                current = &current[pos + 1..];
            } else {
                break;
            }
        }
        Err(Error::Provider("artfiles: couldn't find root domain zone".into()))
    }

    fn get_txt_records(&self, zone: &str, auth: &str) -> Result<String, Error> {
        let url = format!("{}/dns/get_dns.html?domain={}", BASE_URL, zone);
        let resp = http::get(&url, &[("Authorization", auth)])
            .map_err(|e| Error::Provider(format!("artfiles get DNS: {e}")))?;
        if !resp.body.contains("status\":\"OK") {
            return Err(Error::Provider(format!(
                "artfiles get DNS: {}",
                &resp.body[..resp.body.len().min(200)]
            )));
        }
        match serde_json::from_str::<Value>(&resp.body) {
            Ok(v) => match v.get("TXT").and_then(|t| t.as_str()) {
                Some(txt) => Ok(txt.replace("\\n", "\n")),
                None => Ok(String::new()),
            },
            Err(_) => extract_txt_field(&resp.body),
        }
    }

    fn set_txt_records(&self, zone: &str, records: &str, auth: &str) -> Result<(), Error> {
        let encoded = urlencode(records);
        let url = format!("{}/dns/set_dns.html?domain={}&TXT={}", BASE_URL, zone, encoded);
        let resp = http::post(&url, b"", "application/x-www-form-urlencoded", &[("Authorization", auth)])
            .map_err(|e| Error::Provider(format!("artfiles set DNS: {e}")))?;
        if !resp.body.contains("status\":\"OK") {
            return Err(Error::Provider(format!("artfiles set DNS: {}", &resp.body[..resp.body.len().min(200)])));
        }
        Ok(())
    }
}

fn extract_txt_field(body: &str) -> Result<String, Error> {
    let start = body.find("TXT\":\"")
        .ok_or_else(|| Error::Provider("artfiles: no TXT field in response".into()))?;
    let rest = &body[start + 6..];
    let end = rest.find('}').unwrap_or(rest.len());
    let raw = &rest[..end];
    let raw = raw.strip_suffix('"').unwrap_or(raw);
    Ok(raw.replace("\\\"", "\"").replace("\\n", "\n"))
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
