use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://zonomi.com/app/dns/dyndns.jsp";

pub struct Zonomi {
    api_key: String,
}

impl DnsProvider for Zonomi {
    fn slug() -> &'static str {
        "zonomi"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ZONOMI_Api_Key"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("ZONOMI_Api_Key")
            .ok_or_else(|| Error::Config("ZONOMI_Api_Key required".into()))?
            .clone();
        Ok(Box::new(Zonomi { api_key }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let enc_name = url_encode(name);
        let enc_value = url_encode(value);
        let enc_key = url_encode(&self.api_key);
        let url = format!("{BASE_URL}?host={enc_name}&value={enc_value}&api_key={enc_key}&action=SET&type=TXT");
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("zonomi add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("zonomi add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        if resp.body.to_lowercase().contains("error") {
            return Err(Error::Provider(format!("zonomi add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let enc_name = url_encode(name);
        let enc_value = url_encode(value);
        let enc_key = url_encode(&self.api_key);
        let url = format!("{BASE_URL}?host={enc_name}&value={enc_value}&api_key={enc_key}&action=DELETE&type=TXT");
        let _ = http::get(&url, &[]);
        Ok(())
    }
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push(hex((b >> 4) & 0xf));
                out.push(hex(b & 0xf));
            }
        }
    }
    out
}

fn hex(n: u8) -> char {
    if n < 10 {
        (b'0' + n) as char
    } else {
        (b'A' + n - 10) as char
    }
}
