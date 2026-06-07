use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Namecheap {
    api_key: String,
    username: String,
}

impl DnsProvider for Namecheap {
    fn slug() -> &'static str {
        "namecheap"
    }

    fn env_vars() -> &'static [&'static str] {
        &["NAMECHEAP_API_KEY", "NAMECHEAP_USERNAME"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("NAMECHEAP_API_KEY")
            .ok_or_else(|| Error::Config("NAMECHEAP_API_KEY required".into()))?
            .clone();
        let username = env.get("NAMECHEAP_USERNAME")
            .ok_or_else(|| Error::Config("NAMECHEAP_USERNAME required".into()))?
            .clone();
        Ok(Box::new(Namecheap { api_key, username }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let (sld, tld) = split_domain(domain);
        let mut existing = self.get_hosts(sld, tld).unwrap_or_default();
        existing.retain(|h| {
            !(h.host_type == "TXT" && h.name == name && h.address == value)
        });
        existing.push(HostRecord {
            name: name.to_string(),
            host_type: "TXT".to_string(),
            address: value.to_string(),
            ttl: "120".to_string(),
        });
        self.set_hosts(sld, tld, &existing)?;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let (sld, tld) = split_domain(domain);
        let mut existing = match self.get_hosts(sld, tld) {
            Ok(h) => h,
            Err(_) => return Ok(()),
        };
        existing.retain(|h| {
            !(h.host_type == "TXT" && h.name == name && h.address == value)
        });
        let _ = self.set_hosts(sld, tld, &existing);
        Ok(())
    }
}

struct HostRecord {
    name: String,
    host_type: String,
    address: String,
    ttl: String,
}

impl Namecheap {
    fn api_base(&self) -> String {
        format!(
            "https://api.namecheap.com/xml.response?ApiUser={}&ApiKey={}&UserName={}&ClientIp=8.8.8.8",
            urlencode(&self.username),
            urlencode(&self.api_key),
            urlencode(&self.username),
        )
    }

    fn get_hosts(&self, sld: &str, tld: &str) -> Result<Vec<HostRecord>, Error> {
        let url = format!(
            "{}&Command=namecheap.domains.dns.getHosts&SLD={}&TLD={}",
            self.api_base(),
            sld, tld,
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("Namecheap getHosts: {e}")))?;
        Ok(parse_hosts_xml(&resp.body))
    }

    fn set_hosts(&self, sld: &str, tld: &str, hosts: &[HostRecord]) -> Result<(), Error> {
        let mut params = self.api_base();
        params.push_str(&format!(
            "&Command=namecheap.domains.dns.setHosts&SLD={}&TLD={}",
            sld, tld,
        ));
        for (i, h) in hosts.iter().enumerate() {
            let n = i + 1;
            params.push_str(&format!(
                "&HostName{n}={}&RecordType{n}={}&Address{n}={}&TTL{n}={}",
                urlencode(&h.name),
                urlencode(&h.host_type),
                urlencode(&h.address),
                urlencode(&h.ttl),
            ));
        }
        http::post(&params, b"", "application/x-www-form-urlencoded", &[])
            .map_err(|e| Error::Provider(format!("Namecheap setHosts: {e}")))?;
        Ok(())
    }
}

fn split_domain(domain: &str) -> (&str, &str) {
    domain.rsplit_once('.').unwrap_or((domain, ""))
}

fn parse_hosts_xml(xml: &str) -> Vec<HostRecord> {
    let mut records = Vec::new();
    let mut rest = xml.as_bytes();
    while let Some(pos) = rest.windows(5).position(|w| w == b"<Host") {
        rest = &rest[pos..];
        let tag_end = match rest.iter().position(|&b| b == b'>') {
            Some(p) => p + 1,
            None => break,
        };
        let tag = std::str::from_utf8(&rest[..tag_end]).unwrap_or("");
        rest = &rest[tag_end..];
        records.push(HostRecord {
            name: extract_attr(tag, "Name").unwrap_or_default(),
            host_type: extract_attr(tag, "Type").unwrap_or_default(),
            address: extract_attr(tag, "Address").unwrap_or_default(),
            ttl: extract_attr(tag, "TTL").unwrap_or_else(|| "1800".to_string()),
        });
    }
    records
}

fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let prefix = format!("{attr}=\"");
    let start = tag.find(&prefix)? + prefix.len();
    let end = tag[start..].find('"')?;
    Some(tag[start..start + end].to_string())
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
