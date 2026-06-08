use std::collections::HashMap;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Da {
    base_url: String,
    basic_auth: String,
}

impl DnsProvider for Da {
    fn slug() -> &'static str {
        "da"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DA_Api", "DA_Api_Insecure"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_url = env.get("DA_Api")
            .ok_or_else(|| Error::Config("DA_Api required".into()))?
            .clone();
        let (base_url, user, pass) = parse_da_url(&api_url)
            .map_err(|e| Error::Config(format!("DA_Api parse: {e}")))?;
        let creds = format!("{user}:{pass}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Da { base_url, basic_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let full_name = format!("{name}.{domain}.");
        let url = format!(
            "{}/CMD_API_DNS_CONTROL?domain={}&action=add&type=TXT&name={}&value=\"{}\"",
            self.base_url,
            urlencode(domain),
            urlencode(&full_name),
            urlencode(value),
        );
        let resp = http::post(&url, b"", "application/x-www-form-urlencoded", headers)
            .map_err(|e| Error::Provider(format!("DA add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("DA add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        if resp.body.contains("error=1") {
            return Err(Error::Provider(format!("DA add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let full_name = format!("{name}.{domain}.");
        let select_url = format!(
            "{}/CMD_API_DNS_CONTROL?domain={}&action=select[]&type=TXT&name={}",
            self.base_url,
            urlencode(domain),
            urlencode(&full_name),
        );
        let resp = match http::post(&select_url, b"", "application/x-www-form-urlencoded", headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if resp.body.contains("error=1") {
            return Ok(());
        }
        let delete_url = format!(
            "{}/CMD_API_DNS_CONTROL?domain={}&action=delete",
            self.base_url,
            urlencode(domain),
        );
        let _ = http::post(&delete_url, b"", "application/x-www-form-urlencoded", headers);
        Ok(())
    }
}

fn parse_da_url(url: &str) -> Result<(String, String, String), String> {
    let rest = url.strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or("invalid URL scheme")?;
    let (creds, host) = rest.split_once('@')
        .ok_or("no credentials in URL")?;
    let (user, pass) = creds.split_once(':')
        .ok_or("invalid credentials format")?;
    let scheme = if url.starts_with("https://") { "https" } else { "http" };
    let base_url = format!("{scheme}://{host}");
    Ok((base_url, user.to_string(), pass.to_string()))
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
