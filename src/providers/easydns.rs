use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Easydns {
    basic_auth: String,
    api_key: String,
}

impl DnsProvider for Easydns {
    fn slug() -> &'static str {
        "easydns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["EASYDNS_Username", "EASYDNS_Password", "EASYDNS_APIKey"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("EASYDNS_Username")
            .ok_or_else(|| Error::Config("EASYDNS_Username required".into()))?.clone();
        let password = env.get("EASYDNS_Password")
            .ok_or_else(|| Error::Config("EASYDNS_Password required".into()))?.clone();
        let api_key = env.get("EASYDNS_APIKey")
            .ok_or_else(|| Error::Config("EASYDNS_APIKey required".into()))?.clone();
        let creds = format!("{username}:{password}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Easydns { basic_auth, api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let _ = self.resolve_zone(domain)?;
        let url = format!("https://rest.easydns.net/domain/{domain}");
        let form_data = format!("host={}&type=TXT&rdata={}&ttl=120",
            urlencoding(name), urlencoding(value));
        let headers = &[
            ("Authorization", self.basic_auth.as_str()),
            ("X-API-Key", self.api_key.as_str()),
        ];
        http::post(&url, form_data.as_bytes(), "application/x-www-form-urlencoded", headers)
            .map_err(|e| Error::Provider(format!("easydns add TXT: {e}")))?;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let _ = self.resolve_zone(domain)?;
        let record_id = self.find_record_id(domain, name, value)?;
        if let Some(id) = record_id {
            let url = format!("https://rest.easydns.net/domain/{domain}/{id}");
            let headers = &[
                ("Authorization", self.basic_auth.as_str()),
                ("X-API-Key", self.api_key.as_str()),
            ];
            http::delete(&url, headers)
                .map_err(|e| Error::Provider(format!("easydns delete TXT: {e}")))?;
        }
        Ok(())
    }
}

impl Easydns {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let url = "https://rest.easydns.net/domain";
        let headers = &[
            ("Authorization", self.basic_auth.as_str()),
            ("X-API-Key", self.api_key.as_str()),
        ];
        let resp = http::get(url, headers)
            .map_err(|e| Error::Provider(format!("easydns list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("easydns domains: {e}")))?;
        if let Some(arr) = v.as_array() {
            for d in arr {
                if let Some(nm) = d.get("domain").and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        return Ok(nm.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }

    fn find_record_id(&self, domain: &str, name: &str, value: &str) -> Result<Option<String>, Error> {
        let url = format!("https://rest.easydns.net/domain/{domain}");
        let headers = &[
            ("Authorization", self.basic_auth.as_str()),
            ("X-API-Key", self.api_key.as_str()),
        ];
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("easydns list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("easydns records: {e}")))?;
        if let Some(records) = v.get("records").and_then(|r| r.as_array())
            .or_else(|| v.as_array())
        {
            for r in records {
                if r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && r.get("host").and_then(|h| h.as_str()) == Some(name)
                    && r.get("rdata").and_then(|d| d.as_str()) == Some(value)
                {
                    if let Some(id) = r.get("id").and_then(|i| i.as_str()) {
                        return Ok(Some(id.to_string()));
                    }
                }
            }
        }
        Ok(None)
    }
}

fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
