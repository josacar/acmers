use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Dnsexit {
    api_key: String,
}

impl DnsProvider for Dnsexit {
    fn slug() -> &'static str {
        "dnsexit"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DNSEXIT_API_KEY", "DNSEXIT_API_Key"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("DNSEXIT_API_KEY")
            .or_else(|| env.get("DNSEXIT_API_Key"))
            .ok_or_else(|| Error::Config("DNSEXIT_API_KEY required".into()))?
            .clone();
        Ok(Box::new(Dnsexit { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let url = format!(
            "https://api.dnsexit.com/dns/addRecord.jsp?apikey={}&domain={}&name={}&type=TXT&content={}&ttl=120",
            urlencode(&self.api_key),
            urlencode(domain),
            urlencode(name),
            urlencode(value),
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("DNSExit add TXT: {e}")))?;
        if resp.status >= 400 || resp.body.contains("Error") {
            return Err(Error::Provider(format!("DNSExit add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let url = format!(
            "https://api.dnsexit.com/dns/deleteRecord.jsp?apikey={}&domain={}&name={}&type=TXT&content={}",
            urlencode(&self.api_key),
            urlencode(domain),
            urlencode(name),
            urlencode(value),
        );
        let _ = http::get(&url, &[]);
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
