use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Dreamhost {
    api_key: String,
}

impl DnsProvider for Dreamhost {
    fn slug() -> &'static str {
        "dreamhost"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DH_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("DH_API_KEY")
            .ok_or_else(|| Error::Config("DH_API_KEY required".into()))?
            .clone();
        Ok(Box::new(Dreamhost { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let url = format!(
            "https://api.dreamhost.com/?key={}&cmd=dns-add_record&format=json&record={}&type=TXT&value={}",
            self.api_key,
            urlencode(name),
            urlencode(value),
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("DreamHost add TXT: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("DreamHost response: {e}")))?;
        if v.get("result").and_then(|r| r.as_str()) != Some("success") {
            return Err(Error::Provider(format!("DreamHost add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let url = format!(
            "https://api.dreamhost.com/?key={}&cmd=dns-remove_record&format=json&record={}&type=TXT&value={}",
            self.api_key,
            urlencode(name),
            urlencode(value),
        );
        http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("DreamHost remove TXT: {e}")))?;
        Ok(())
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => {
                let upper = (b >> 4) & 0xf;
                let lower = b & 0xf;
                out.push('%');
                out.push(HEX[upper as usize] as char);
                out.push(HEX[lower as usize] as char);
            }
        }
    }
    out
}

const HEX: &[u8] = b"0123456789ABCDEF";
