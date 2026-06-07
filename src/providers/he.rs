use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct He {
    username: String,
    password: String,
}

impl DnsProvider for He {
    fn slug() -> &'static str {
        "he"
    }

    fn env_vars() -> &'static [&'static str] {
        &["HE_Username", "HE_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(He {
            username: env.get("HE_Username")
                .ok_or_else(|| Error::Config("HE_Username required".into()))?
                .clone(),
            password: env.get("HE_Password")
                .ok_or_else(|| Error::Config("HE_Password required".into()))?
                .clone(),
        }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let cookies = self.login()?;
        self.find_zone_id(domain, &cookies)?;

        let headers: &[(&str, &str)] = &[("Cookie", &cookies)];

        let zone_page_url = format!("https://dns.he.net/?host={domain}");
        let resp = http::get(&zone_page_url, headers)
            .map_err(|e| Error::Provider(format!("HE zone page: {e}")))?;
        let body = &resp.body;

        let account = extract_value(body, "name=\"account\" value=\"", "\"")
            .ok_or_else(|| Error::Provider("HE: could not find account token".into()))?;

        let form = format!(
            "account={}&host={}&type=TXT&name={}&value={}&ttl=300&submit=Save",
            url_encode_f(&account),
            url_encode_f(domain),
            url_encode_f(name),
            url_encode_f(value),
        );
        let post_resp = http::post(
            &zone_page_url,
            form.as_bytes(),
            "application/x-www-form-urlencoded",
            headers,
        )
        .map_err(|e| Error::Provider(format!("HE add TXT: {e}")))?;
        if post_resp.status >= 400 {
            return Err(Error::Provider(format!("HE add TXT: HTTP {}", post_resp.status)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let cookies = match self.login() {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };
        if self.find_zone_id(domain, &cookies).is_err() {
            return Ok(());
        }

        let headers: &[(&str, &str)] = &[("Cookie", &cookies)];
        let zone_page_url = format!("https://dns.he.net/?host={domain}");
        let resp = match http::get(&zone_page_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };

        let body = &resp.body;
        let account = match extract_value(body, "name=\"account\" value=\"", "\"") {
            Some(a) => a,
            None => return Ok(()),
        };

        let del_marker = format!("name={name}&type=TXT");
        if let Some(block) = find_block_before(body, &del_marker, "<tr") {
            if let Some(del_id) = extract_value(&block, "data-del=\"", "\"") {
                let del_form = format!(
                    "account={}&host={}&data_del={}&submit=Delete",
                    url_encode_f(&account),
                    url_encode_f(domain),
                    url_encode_f(&del_id),
                );
                let _ = http::post(
                    &zone_page_url,
                    del_form.as_bytes(),
                    "application/x-www-form-urlencoded",
                    headers,
                );
            }
        }
        Ok(())
    }
}

impl He {
    fn login(&self) -> Result<String, Error> {
        let _ = http::get("https://dns.he.net/", &[]);

        let form = format!(
            "email={}&pass={}&submit=Login",
            url_encode_f(&self.username),
            url_encode_f(&self.password),
        );
        let resp = http::post(
            "https://dns.he.net/",
            form.as_bytes(),
            "application/x-www-form-urlencoded",
            &[],
        )
        .map_err(|e| Error::Provider(format!("HE login: {e}")))?;

        let cookies = resp.headers.get("set-cookie")
            .cloned()
            .unwrap_or_default();

        if resp.body.contains("Login failed") || resp.body.contains("Invalid") {
            return Err(Error::Provider("HE: login failed".into()));
        }

        Ok(parse_cookies_he(&cookies))
    }

    fn find_zone_id(&self, domain: &str, cookies: &str) -> Result<String, Error> {
        let headers: &[(&str, &str)] = &[("Cookie", cookies)];
        let resp = http::get("https://dns.he.net/", headers)
            .map_err(|e| Error::Provider(format!("HE list zones: {e}")))?;

        if resp.body.contains(domain) {
            return Ok(domain.to_string());
        }

        Err(Error::Provider(format!("HE: zone not found for {domain}")))
    }
}

fn find_block_before<'a>(text: &'a str, marker: &str, boundary: &str) -> Option<String> {
    if let Some(pos) = text.find(marker) {
        let before = &text[..pos];
        if let Some(bpos) = before.rfind(boundary) {
            return Some(before[bpos..].to_string());
        }
        return Some(before[before.len().saturating_sub(1024)..].to_string());
    }
    None
}

fn extract_value(text: &str, prefix: &str, suffix: &str) -> Option<String> {
    let start = text.find(prefix)? + prefix.len();
    let end = text[start..].find(suffix)?;
    Some(text[start..start + end].to_string())
}

fn url_encode_f(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push(hex_f((b >> 4) & 0xf));
                out.push(hex_f(b & 0xf));
            }
        }
    }
    out
}

fn hex_f(n: u8) -> char {
    if n < 10 {
        (b'0' + n) as char
    } else {
        (b'A' + n - 10) as char
    }
}

fn parse_cookies_he(raw: &str) -> String {
    let mut cookies = Vec::new();
    for part in raw.split(',') {
        let part = part.trim();
        if let Some(semi) = part.find(';') {
            if part[..semi].contains('=') {
                cookies.push(part[..semi].to_string());
            }
        } else if part.contains('=') {
            cookies.push(part.to_string());
        }
    }
    cookies.join("; ")
}
