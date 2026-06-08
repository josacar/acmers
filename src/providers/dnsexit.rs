use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const API_URL: &str = "https://api.dnsexit.com/dns/";
const HOSTS_URL: &str = "https://update.dnsexit.com/ipupdate/hosts.jsp";

pub struct Dnsexit {
    api_key: String,
    auth_user: String,
    auth_pass: String,
}

impl DnsProvider for Dnsexit {
    fn slug() -> &'static str {
        "dnsexit"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DNSEXIT_API_KEY", "DNSEXIT_AUTH_USER", "DNSEXIT_AUTH_PASS"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("DNSEXIT_API_KEY")
            .ok_or_else(|| Error::Config("DNSEXIT_API_KEY required".into()))?
            .clone();
        let auth_user = env.get("DNSEXIT_AUTH_USER")
            .ok_or_else(|| Error::Config("DNSEXIT_AUTH_USER required".into()))?
            .clone();
        let auth_pass = env.get("DNSEXIT_AUTH_PASS")
            .ok_or_else(|| Error::Config("DNSEXIT_AUTH_PASS required".into()))?
            .clone();
        Ok(Box::new(Dnsexit { api_key, auth_user, auth_pass }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (sub_domain, root) = self.get_root(name)?;
        let body = serde_json::json!({
            "domain": root,
            "add": {
                "type": "TXT",
                "name": sub_domain,
                "content": value,
                "ttl": 0,
                "overwrite": false
            }
        });
        let resp = http::post(
            API_URL,
            &serde_json::to_vec(&body).unwrap(),
            "application/json",
            &[("apikey", &self.api_key)],
        )
        .map_err(|e| Error::Provider(format!("DNSExit add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("DNSExit add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (sub_domain, root) = self.get_root(name)?;
        let body = serde_json::json!({
            "domain": root,
            "delete": {
                "type": "TXT",
                "name": sub_domain,
                "content": value
            }
        });
        let resp = http::post(
            API_URL,
            &serde_json::to_vec(&body).unwrap(),
            "application/json",
            &[("apikey", &self.api_key)],
        )
        .map_err(|e| Error::Provider(format!("DNSExit remove TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("DNSExit remove TXT: {}", resp.body)));
        }
        Ok(())
    }
}

impl Dnsexit {
    fn get_root(&self, fulldomain: &str) -> Result<(String, String), Error> {
        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 0..parts.len() {
            let domain = parts[i..].join(".");
            let url = format!(
                "{}?login={}&password={}&domain={}",
                HOSTS_URL,
                urlencode(&self.auth_user),
                urlencode(&self.auth_pass),
                urlencode(&domain),
            );
            let resp = http::get(&url, &[])
                .map_err(|e| Error::Provider(format!("DNSExit zone resolution: {e}")))?;
            let needle = format!("0={domain}");
            if resp.body.contains(&needle) {
                let sub_domain = if i == 0 {
                    String::new()
                } else {
                    parts[..i].join(".")
                };
                return Ok((sub_domain, domain));
            }
        }
        Err(Error::Provider(format!(
            "DNSExit: could not resolve root zone for {fulldomain}"
        )))
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
