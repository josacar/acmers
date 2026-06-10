use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const API_URL: &str = "https://coreapi.1api.net/api/call.cgi";

pub struct Hexonet {
    login: String,
    password: String,
}

impl DnsProvider for Hexonet {
    fn slug() -> &'static str {
        "hexonet"
    }

    fn env_vars() -> &'static [&'static str] {
        &["HEXONET_User", "HEXONET_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let login = env.get("HEXONET_User")
            .ok_or_else(|| Error::Config("HEXONET_User required".into()))?
            .clone();
        let password = env.get("HEXONET_Password")
            .ok_or_else(|| Error::Config("HEXONET_Password required".into()))?
            .clone();
        if !login.contains('!') {
            return Err(Error::Config("HEXONET_User must be a restricted user (contain '!')".into()));
        }
        Ok(Box::new(Hexonet { login, password }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (sub_domain, zone) = self.get_root(name)?;
        let query = format!(
            "command=QueryDNSZoneRRList&dnszone={}.&RRTYPE=TXT",
            url_encode(&zone),
        );
        let resp = self.api_call(&query)?;
        if !resp.contains("CODE=200") {
            return Err(Error::Provider(format!("hexonet query zone: {}", resp)));
        }
        let add_query = format!(
            "command=UpdateDNSZone&dnszone={}.&addrr0={} IN TXT {}",
            url_encode(&zone),
            url_encode(&sub_domain),
            url_encode(value),
        );
        let resp = self.api_call(&add_query)?;
        if !resp.contains("CODE=200") {
            return Err(Error::Provider(format!("hexonet add TXT: {}", resp)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (sub_domain, zone) = match self.get_root(name) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("warning: hexonet cleanup zone not found: {e}");
                return Ok(());
            }
        };
        let query = format!(
            "command=QueryDNSZoneRRList&dnszone={}.&RRTYPE=TXT&RR={} IN TXT \"{}\"",
            url_encode(&zone),
            url_encode(&sub_domain),
            url_encode(value),
        );
        let resp = match self.api_call(&query) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("warning: hexonet cleanup query failed: {e}");
                return Ok(());
            }
        };
        if !resp.contains("CODE=200") {
            eprintln!("warning: hexonet cleanup query error: {}", resp);
            return Ok(());
        }
        let count = extract_property(&resp, "TOTAL").unwrap_or_default();
        if count == "0" {
            return Ok(());
        }
        let del_query = format!(
            "command=UpdateDNSZone&dnszone={}.&delrr0={} IN TXT \"{}\"",
            url_encode(&zone),
            url_encode(&sub_domain),
            url_encode(value),
        );
        let resp = self.api_call(&del_query)?;
        if !resp.contains("CODE=200") {
            return Err(Error::Provider(format!("hexonet del TXT: {}", resp)));
        }
        Ok(())
    }
}

impl Hexonet {
    fn api_call(&self, query: &str) -> Result<String, Error> {
        let url = format!(
            "{}?s_login={}&s_pw={}&{}",
            API_URL,
            url_encode(&self.login),
            url_encode(&self.password),
            query,
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("hexonet API: {e}")))?;
        Ok(resp.body)
    }

    fn get_root(&self, fulldomain: &str) -> Result<(String, String), Error> {
        let labels: Vec<&str> = fulldomain.split('.').collect();
        for i in 0..labels.len() {
            let zone = labels[i..].join(".");
            if zone.is_empty() {
                break;
            }
            let query = format!("command=QueryDNSZoneRRList&dnszone={}.", url_encode(&zone));
            let resp = self.api_call(&query)?;
            if resp.contains("CODE=200") {
                let sub_domain = labels[..i].join(".");
                return Ok((sub_domain, zone));
            }
        }
        Err(Error::Provider(format!("hexonet: zone not found for {fulldomain}")))    }
}

fn extract_property(body: &str, name: &str) -> Option<String> {
    let prefix = format!("PROPERTY[{name}][0]=");
    for line in body.lines() {
        if let Some(pos) = line.find(&prefix) {
            let start = pos + prefix.len();
            return Some(line[start..].trim().to_string());
        }
    }
    None
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push_str("%20"),
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
    let n = n & 0xf;
    if n < 10 { (b'0' + n) as char } else { (b'A' + n - 10) as char }
}
