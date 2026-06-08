use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const API_BASE: &str = "https://api.sitehost.nz/1.5";

pub struct Sitehost {
    api_key: String,
    client_id: String,
}

impl DnsProvider for Sitehost {
    fn slug() -> &'static str {
        "sitehost"
    }

    fn env_vars() -> &'static [&'static str] {
        &["SITEHOST_API_KEY", "SITEHOST_CLIENT_ID"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("SITEHOST_API_KEY")
            .ok_or_else(|| Error::Config("SITEHOST_API_KEY required".into()))?
            .clone();
        let client_id = env.get("SITEHOST_CLIENT_ID")
            .ok_or_else(|| Error::Config("SITEHOST_CLIENT_ID required".into()))?
            .clone();
        Ok(Box::new(Sitehost { api_key, client_id }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = self.resolve_zone(name)?;
        let body = format!(
            "apikey={}&client_id={}&domain={}&type=TXT&name={}&content={}",
            urlencode(&self.api_key),
            urlencode(&self.client_id),
            urlencode(&zone),
            urlencode(name),
            urlencode(value),
        );
        let resp = http::post(
            &format!("{API_BASE}/dns/add_record.json"),
            body.as_bytes(),
            "application/x-www-form-urlencoded",
            &[],
        ).map_err(|e| Error::Provider(format!("SiteHost add TXT: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("SiteHost add TXT parse: {e}")))?;
        if !v.get("status").and_then(|s| s.as_bool()).unwrap_or(false) {
            return Err(Error::Provider(format!("SiteHost add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = match self.resolve_zone(name) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let list_url = format!(
            "{API_BASE}/dns/list_records.json?apikey={}&client_id={}&domain={}",
            urlencode(&self.api_key),
            urlencode(&self.client_id),
            urlencode(&zone),
        );
        let resp = match http::get(&list_url, &[]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if !v.get("status").and_then(|s| s.as_bool()).unwrap_or(false) {
            return Ok(());
        }
        let records = v.get("return").and_then(|r| r.as_array());
        if let Some(arr) = records {
            for record in arr {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_body = format!(
                            "apikey={}&client_id={}&domain={}&record_id={}",
                            urlencode(&self.api_key),
                            urlencode(&self.client_id),
                            urlencode(&zone),
                            urlencode(&id),
                        );
                        let _ = http::post(
                            &format!("{API_BASE}/dns/delete_record.json"),
                            del_body.as_bytes(),
                            "application/x-www-form-urlencoded",
                            &[],
                        );
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Sitehost {
    fn resolve_zone(&self, fulldomain: &str) -> Result<String, Error> {
        let mut all_domains: Vec<String> = Vec::new();
        let mut page = 1u32;
        loop {
            let url = format!(
                "{API_BASE}/dns/list_domains.json?apikey={}&client_id={}&filters%5Bpage_number%5D={}",
                urlencode(&self.api_key),
                urlencode(&self.client_id),
                page,
            );
            let resp = http::get(&url, &[])
                .map_err(|e| Error::Provider(format!("SiteHost list domains: {e}")))?;
            let v: Value = serde_json::from_str(&resp.body)
                .map_err(|e| Error::Json(format!("SiteHost list domains parse: {e}")))?;
            if !v.get("status").and_then(|s| s.as_bool()).unwrap_or(false) {
                return Err(Error::Provider(format!("SiteHost list domains: {}", resp.body)));
            }
            if let Some(ret) = v.get("return") {
                if let Some(arr) = ret.as_array() {
                    for item in arr {
                        if let Some(d) = item.as_str() {
                            all_domains.push(d.to_string());
                        } else if let Some(d) = item.get("domain").and_then(|v| v.as_str()) {
                            all_domains.push(d.to_string());
                        } else if let Some(d) = item.get("name").and_then(|v| v.as_str()) {
                            all_domains.push(d.to_string());
                        }
                    }
                } else if let Some(arr) = ret.get("domains").and_then(|d| d.as_array()) {
                    for item in arr {
                        if let Some(d) = item.as_str() {
                            all_domains.push(d.to_string());
                        } else if let Some(d) = item.get("domain").and_then(|v| v.as_str()) {
                            all_domains.push(d.to_string());
                        } else if let Some(d) = item.get("name").and_then(|v| v.as_str()) {
                            all_domains.push(d.to_string());
                        }
                    }
                }
            }
            let total_pages = v.get("total_pages").and_then(|t| t.as_u64()).unwrap_or(1);
            if page as u64 >= total_pages {
                break;
            }
            page += 1;
        }

        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 0..parts.len() {
            let candidate = parts[i..].join(".");
            if all_domains.iter().any(|d| d == &candidate) {
                return Ok(candidate);
            }
        }
        Err(Error::Provider(format!("SiteHost: no zone found for {fulldomain}")))
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
