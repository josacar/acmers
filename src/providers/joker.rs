use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const JOKER_API: &str = "https://svc.joker.com/nic/replace";

pub struct Joker {
    username: String,
    password: String,
}

impl DnsProvider for Joker {
    fn slug() -> &'static str {
        "joker"
    }

    fn env_vars() -> &'static [&'static str] {
        &["JOKER_USERNAME", "JOKER_PASSWORD"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("JOKER_USERNAME")
            .ok_or_else(|| Error::Config("JOKER_USERNAME required".into()))?.clone();
        let password = env.get("JOKER_PASSWORD")
            .ok_or_else(|| Error::Config("JOKER_PASSWORD required".into()))?.clone();
        Ok(Box::new(Joker { username, password }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone, label) = self.get_root(name)?;
        let form = format!(
            "username={}&password={}&zone={}&label={}&type=TXT&value={}",
            url_encode(&self.username),
            url_encode(&self.password),
            url_encode(&zone),
            url_encode(&label),
            url_encode(value),
        );
        let resp = http::post(JOKER_API, form.as_bytes(), "application/x-www-form-urlencoded", &[])
            .map_err(|e| Error::Provider(format!("joker add TXT: {e}")))?;
        if !resp.body.starts_with("OK") {
            return Err(Error::Provider(format!("joker add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, _value: &str) -> ProviderResult {
        let (zone, label) = self.get_root(name)?;
        let form = format!(
            "username={}&password={}&zone={}&label={}&type=TXT&value=",
            url_encode(&self.username),
            url_encode(&self.password),
            url_encode(&zone),
            url_encode(&label),
        );
        let resp = http::post(JOKER_API, form.as_bytes(), "application/x-www-form-urlencoded", &[])
            .map_err(|e| Error::Provider(format!("joker remove TXT: {e}")))?;
        if !resp.body.starts_with("OK") {
            eprintln!("warning: joker remove TXT: {}", resp.body);
        }
        Ok(())
    }
}

impl Joker {
    fn get_root(&self, fulldomain: &str) -> Result<(String, String), Error> {
        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 0..parts.len() {
            let candidate = parts[i..].join(".");
            if candidate.is_empty() {
                continue;
            }
            let form = format!(
                "username={}&password={}&zone={}&label=jokerTXTUpdateTest&type=TXT&value=",
                url_encode(&self.username),
                url_encode(&self.password),
                url_encode(&candidate),
            );
            if let Ok(resp) = http::post(JOKER_API, form.as_bytes(), "application/x-www-form-urlencoded", &[]) {
                if resp.body.starts_with("OK") {
                    let sub_domain = if i == 0 {
                        String::new()
                    } else {
                        parts[..i].join(".")
                    };
                    return Ok((candidate, sub_domain));
                }
            }
        }
        Err(Error::Provider(format!("joker: root domain not found for {fulldomain}")))
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
                out.push(hex_char((b >> 4) & 0xf));
                out.push(hex_char(b & 0xf));
            }
        }
    }
    out
}

fn hex_char(n: u8) -> char {
    if n < 10 { (b'0' + n) as char } else { (b'a' + n - 10) as char }
}
