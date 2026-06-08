use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const API_URL: &str = "https://selfhost.de/cgi-bin/api.pl";

pub struct Selfhost {
    username: String,
    password: String,
    map: String,
}

impl DnsProvider for Selfhost {
    fn slug() -> &'static str {
        "selfhost"
    }

    fn env_vars() -> &'static [&'static str] {
        &["SELFHOSTDNS_USERNAME", "SELFHOSTDNS_PASSWORD", "SELFHOSTDNS_MAP"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("SELFHOSTDNS_USERNAME")
            .ok_or_else(|| Error::Config("SELFHOSTDNS_USERNAME required".into()))?
            .clone();
        let password = env.get("SELFHOSTDNS_PASSWORD")
            .ok_or_else(|| Error::Config("SELFHOSTDNS_PASSWORD required".into()))?
            .clone();
        let map = env.get("SELFHOSTDNS_MAP")
            .ok_or_else(|| Error::Config("SELFHOSTDNS_MAP required".into()))?
            .clone();
        Ok(Box::new(Selfhost { username, password, map }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let rid = find_rid(&self.map, name)
            .ok_or_else(|| Error::Provider(format!("selfhost: no RID found for {name} in SELFHOSTDNS_MAP")))?;

        let url = format!(
            "{}?username={}&password={}&rid={}&content={}",
            API_URL,
            url_encode(&self.username),
            url_encode(&self.password),
            rid,
            url_encode(value),
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("selfhost add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("selfhost add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}

fn find_rid(map: &str, fulldomain: &str) -> Option<String> {
    for entry in map.split_whitespace() {
        let parts: Vec<&str> = entry.splitn(3, ':').collect();
        if parts.len() >= 2 && parts[0] == fulldomain {
            return Some(parts[1].to_string());
        }
    }
    None
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push(hex_char((b >> 4) & 0xf));
                out.push(hex_char(b & 0xf));
            }
        }
    }
    out
}

fn hex_char(n: u8) -> char {
    if n < 10 {
        (b'0' + n) as char
    } else {
        (b'a' + n - 10) as char
    }
}
