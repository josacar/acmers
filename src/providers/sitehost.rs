use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Sitehost {
    basic_auth: String,
    api_key: String,
}

impl DnsProvider for Sitehost {
    fn slug() -> &'static str {
        "sitehost"
    }

    fn env_vars() -> &'static [&'static str] {
        &["SITEHOST_ApiKey", "SITEHOST_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("SITEHOST_ApiKey")
            .ok_or_else(|| Error::Config("SITEHOST_ApiKey required".into()))?
            .clone();
        let secret = env.get("SITEHOST_Secret")
            .ok_or_else(|| Error::Config("SITEHOST_Secret required".into()))?
            .clone();
        let creds = format!("{api_key}:{secret}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Sitehost { basic_auth, api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let body = format!(
            "client_id={}&apikey={}&domain={}&type=TXT&name={}&content={}&ttl=120",
            urlencode(&self.api_key),
            urlencode(&self.api_key),
            urlencode(domain),
            urlencode(name),
            urlencode(value),
        );
        let resp = http::post(
            "https://api.sitehost.nz/1.3/dns/add_record.json",
            body.as_bytes(),
            "application/x-www-form-urlencoded",
            headers,
        ).map_err(|e| Error::Provider(format!("SiteHost add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("SiteHost add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let list_url = format!(
            "https://api.sitehost.nz/1.3/dns/list_records.json?client_id={}&domain={}",
            urlencode(&self.api_key),
            urlencode(domain),
        );
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records: Option<&Vec<Value>> = v.get("data").and_then(|r| {
            if let Some(arr) = r.as_array() { Some(arr) } else { r.get("records").and_then(|a| a.as_array()) }
        });
        if let Some(arr) = records {
            for record in arr {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").or(record.get("record_id")).and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_body = format!(
                            "client_id={}&apikey={}&domain={}&record_id={}",
                            urlencode(&self.api_key),
                            urlencode(&self.api_key),
                            urlencode(domain),
                            urlencode(&id),
                        );
                        let _ = http::post(
                            "https://api.sitehost.nz/1.3/dns/delete_record.json",
                            del_body.as_bytes(),
                            "application/x-www-form-urlencoded",
                            headers,
                        );
                        return Ok(());
                    }
                }
            }
        }
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
