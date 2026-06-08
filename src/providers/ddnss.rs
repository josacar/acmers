use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const SUFFIXES: &[&str] = &[
    "ddnss", "dyn-ip24", "dyndns", "dyn", "dyndns1",
    "home-webserver", "myhome-server", "dynip",
];

pub struct Ddnss {
    token: String,
}

fn extract_domain(fulldomain: &str) -> Result<String, Error> {
    let lower = fulldomain.to_lowercase();
    let parts: Vec<&str> = lower.split('.').collect();
    for (i, part) in parts.iter().enumerate() {
        if SUFFIXES.contains(part) && i >= 2 && i + 1 < parts.len() {
            return Ok(parts[i - 1..].join("."));
        }
    }
    Err(Error::Provider(format!(
        "ddnss: cannot extract domain from fulldomain: {fulldomain}"
    )))
}

impl DnsProvider for Ddnss {
    fn slug() -> &'static str {
        "ddnss"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DDNSS_Token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("DDNSS_Token")
            .ok_or_else(|| Error::Config("DDNSS_Token required".into()))?
            .clone();
        Ok(Box::new(Ddnss { token }))
    }

    fn add_txt(&self, domain: &str, _name: &str, value: &str) -> ProviderResult {
        let ddnss_domain = extract_domain(domain)?;
        let url = format!(
            "https://ddnss.de/upd.php?key={}&host={}&txtm=1&txt={}",
            self.token, ddnss_domain, value
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("ddnss add TXT: {e}")))?;
        let body = resp.body.trim();
        if !body.contains("Updated") {
            return Err(Error::Provider(format!(
                "ddnss add TXT: unexpected response: {body}"
            )));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, _name: &str, _value: &str) -> ProviderResult {
        let ddnss_domain = extract_domain(domain)?;
        let url = format!(
            "https://ddnss.de/upd.php?key={}&host={}&txtm=2",
            self.token, ddnss_domain
        );
        let resp = match http::get(&url, &[]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let body = resp.body.trim();
        if !body.contains("Updated") {
            eprintln!("warning: ddnss remove TXT: unexpected response: {body}");
        }
        Ok(())
    }
}
