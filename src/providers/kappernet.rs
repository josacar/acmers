use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Kappernet {
    api_key: String,
    api_secret: String,
}

impl DnsProvider for Kappernet {
    fn slug() -> &'static str {
        "kappernet"
    }

    fn env_vars() -> &'static [&'static str] {
        &["KAPPERNETDNS_Key", "KAPPERNETDNS_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("KAPPERNETDNS_Key")
            .ok_or_else(|| Error::Config("KAPPERNETDNS_Key required".into()))?
            .clone();
        let api_secret = env.get("KAPPERNETDNS_Secret")
            .ok_or_else(|| Error::Config("KAPPERNETDNS_Secret required".into()))?
            .clone();
        Ok(Box::new(Kappernet { api_key, api_secret }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = self.resolve_zone(name)?;
        let data = format!(
            r#"{{"name":"{}","type":"TXT","content":"{}","ttl":"300","prio":""}}"#,
            name, value
        );
        let encoded_data = urlencode(&data);
        let url = format!(
            "https://dnspanel.kapper.net/API/1.2?APIKey={}&APISecret={}&action=new&subject={}&data={}",
            self.api_key, self.api_secret, zone, encoded_data
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("kappernet add TXT: {e}")))?;
        if !resp.body.contains(r#"{"OK":true"#) {
            return Err(Error::Provider(format!("kappernet add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let data = format!(
            r#"{{"name":"{}","type":"TXT","content":"{}","ttl":"300","prio":""}}"#,
            name, value
        );
        let encoded_data = urlencode(&data);
        let url = format!(
            "https://dnspanel.kapper.net/API/1.2?APIKey={}&APISecret={}&action=del&subject={}&data={}",
            self.api_key, self.api_secret, name, encoded_data
        );
        let _ = http::get(&url, &[]);
        Ok(())
    }
}

impl Kappernet {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 1..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                break;
            }
            let url = format!(
                "https://dnspanel.kapper.net/API/1.2?APIKey={}&APISecret={}&action=list&subject={}",
                self.api_key, self.api_secret, h
            );
            match http::get(&url, &[]) {
                Ok(resp) => {
                    if !resp.body.contains(r#""OK":false"#) {
                        return Ok(h);
                    }
                }
                Err(_) => continue,
            }
        }
        Err(Error::Provider(format!("kappernet: could not resolve zone for {domain}")))
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
