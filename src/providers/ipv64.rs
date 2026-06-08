use std::collections::HashMap;
use std::thread;
use std::time::Duration;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Ipv64 {
    token: String,
}

impl DnsProvider for Ipv64 {
    fn slug() -> &'static str {
        "ipv64"
    }

    fn env_vars() -> &'static [&'static str] {
        &["IPv64_Token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("IPv64_Token")
            .ok_or_else(|| Error::Config("IPv64_Token required".into()))?.clone();
        Ok(Box::new(Ipv64 { token }))
    }

    fn add_txt(&self, domain: &str, _name: &str, value: &str) -> ProviderResult {
        let (zone, sub) = self.resolve_zone(domain)?;
        let zone = zone.to_lowercase();
        let sub = sub.to_lowercase();
        let auth = format!("Bearer {}", self.token);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let body = format!("add_record={}&praefix={}&type=TXT&content={}", zone, sub, value);
        let resp = http::post("https://ipv64.net/api", body.as_bytes(), "application/x-www-form-urlencoded", headers)
            .map_err(|e| Error::Provider(format!("ipv64 add TXT: {e}")))?;
        let resp = if resp.body.contains("429 Too Many Requests") {
            thread::sleep(Duration::from_secs(10));
            http::post("https://ipv64.net/api", body.as_bytes(), "application/x-www-form-urlencoded", headers)
                .map_err(|e| Error::Provider(format!("ipv64 add TXT: {e}")))?
        } else {
            resp
        };
        if !resp.body.contains("\"info\":\"success\"") {
            return Err(Error::Provider(format!("ipv64 add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, _name: &str, value: &str) -> ProviderResult {
        let (zone, sub) = match self.resolve_zone(domain) {
            Ok(v) => v,
            Err(e) => { eprintln!("warning: cleanup failed: {e}"); return Ok(()); }
        };
        let zone = zone.to_lowercase();
        let sub = sub.to_lowercase();
        let auth = format!("Bearer {}", self.token);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let body = format!("del_record={}&praefix={}&type=TXT&content={}", zone, sub, value);
        let resp = match http::delete_with_body("https://ipv64.net/api", body.as_bytes(), "application/x-www-form-urlencoded", headers) {
            Ok(r) => r,
            Err(e) => { eprintln!("warning: cleanup failed: {e}"); return Ok(()); }
        };
        if resp.body.contains("429 Too Many Requests") {
            thread::sleep(Duration::from_secs(10));
            let _ = http::delete_with_body("https://ipv64.net/api", body.as_bytes(), "application/x-www-form-urlencoded", headers);
        }
        Ok(())
    }
}

impl Ipv64 {
    fn resolve_zone(&self, fulldomain: &str) -> Result<(String, String), Error> {
        let auth = format!("Bearer {}", self.token);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let resp = http::get("https://ipv64.net/api?get_domains", headers)
            .map_err(|e| Error::Provider(format!("ipv64 get_domains: {e}")))?;
        let mut body = resp.body;
        if body.contains("429 Too Many Requests") {
            thread::sleep(Duration::from_secs(10));
            let resp = http::get("https://ipv64.net/api?get_domains", headers)
                .map_err(|e| Error::Provider(format!("ipv64 get_domains: {e}")))?;
            body = resp.body;
        }
        let v: Value = serde_json::from_str(&body)
            .map_err(|e| Error::Provider(format!("ipv64 get_domains parse: {e}")))?;
        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 0..parts.len() {
            let candidate = parts[i..].join(".");
            if v.get(&candidate).is_some() {
                let sub = parts[..i].join(".");
                return Ok((candidate, sub));
            }
        }
        Err(Error::Provider(format!("ipv64: no zone found for {}", fulldomain)))
    }
}
