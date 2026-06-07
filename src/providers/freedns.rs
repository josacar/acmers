use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Freedns {
    username: String,
    password: String,
}

impl DnsProvider for Freedns {
    fn slug() -> &'static str {
        "freedns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["FREEDNS_User", "FREEDNS_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Freedns {
            username: env.get("FREEDNS_User")
                .ok_or_else(|| Error::Config("FREEDNS_User required".into()))?
                .clone(),
            password: env.get("FREEDNS_Password")
                .ok_or_else(|| Error::Config("FREEDNS_Password required".into()))?
                .clone(),
        }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let cookies = self.login()?;
        let domain_id = self.find_domain_id(domain, &cookies)?;
        let subdomain = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);

        let form = format!(
            "type=TXT&subdomain={}&address={}&ttl=300&submit=Save",
            url_encode(subdomain),
            url_encode(value),
        );
        let url = format!("https://freedns.afraid.org/subdomain/save.php?edit_domain_id={domain_id}");
        let headers: &[(&str, &str)] = &[("Cookie", &cookies)];
        let resp = http::post(&url, form.as_bytes(), "application/x-www-form-urlencoded", headers)
            .map_err(|e| Error::Provider(format!("freedns add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("freedns add TXT: HTTP {}", resp.status)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let cookies = match self.login() {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };
        let domain_id = match self.find_domain_id(domain, &cookies) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let subdomain = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);

        let records = self.list_records(&domain_id, &cookies);
        for (rid, rtype, rname, rval) in records {
            if rtype == "TXT" && rname == subdomain && rval == value {
                let del_url = format!("https://freedns.afraid.org/subdomain/delete2.php?data_del_id={rid}");
                let headers: &[(&str, &str)] = &[("Cookie", &cookies)];
                let _ = http::get(&del_url, headers);
                return Ok(());
            }
        }
        Ok(())
    }
}

impl Freedns {
    fn login(&self) -> Result<String, Error> {
        let form = format!(
            "username={}&password={}&submit=Login&action=auth",
            url_encode(&self.username),
            url_encode(&self.password),
        );
        let resp = http::post(
            "https://freedns.afraid.org/zc.php?step=2",
            form.as_bytes(),
            "application/x-www-form-urlencoded",
            &[],
        )
        .map_err(|e| Error::Provider(format!("freedns login: {e}")))?;

        let cookies = resp.headers.get("set-cookie")
            .cloned()
            .unwrap_or_default();

        if cookies.is_empty() {
            return Err(Error::Provider("freedns: login failed - no session cookie".into()));
        }

        Ok(parse_cookies(&cookies))
    }

    fn find_domain_id(&self, domain: &str, cookies: &str) -> Result<String, Error> {
        let headers: &[(&str, &str)] = &[("Cookie", cookies)];
        let resp = http::get("https://freedns.afraid.org/subdomain/", headers)
            .map_err(|e| Error::Provider(format!("freedns list subdomains: {e}")))?;

        let body = &resp.body;
        let search = format!("edit_domain_id=");
        let mut pos = 0;
        while let Some(idx) = body[pos..].find(&search) {
            let start = pos + idx + search.len();
            let end = body[start..].find(|c: char| !c.is_ascii_digit()).unwrap_or(body[start..].len());
            let did = &body[start..start + end];

            if body[start..].contains(domain) {
                return Ok(did.to_string());
            }
            pos = start + end;
        }

        let direct = format!("edit_domain_id=");
        if let Some(idx) = body.find(&direct) {
            let start = idx + direct.len();
            let end = body[start..].find(|c: char| !c.is_ascii_digit()).unwrap_or(body[start..].len());
            let did = &body[start..start + end];
            if !did.is_empty() {
                return Ok(did.to_string());
            }
        }

        Err(Error::Provider(format!("freedns: domain {domain} not found")))
    }

    fn list_records(&self, domain_id: &str, cookies: &str) -> Vec<(String, String, String, String)> {
        let headers: &[(&str, &str)] = &[("Cookie", cookies)];
        let url = format!("https://freedns.afraid.org/subdomain/edit.php?edit_domain_id={domain_id}");
        let resp = match http::get(&url, headers) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        let mut records = Vec::new();
        let body = &resp.body;

        let mut pos = 0;
        while let Some(idx) = body[pos..].find("data_del_id=") {
            let start = pos + idx + "data_del_id=".len();
            let end = body[start..].find(|c: char| !c.is_ascii_digit()).unwrap_or(body[start..].len());
            let rid = body[start..start + end].to_string();
            pos = start + end;
            records.push((rid, "TXT".to_string(), String::new(), String::new()));
        }

        records
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

fn parse_cookies(raw: &str) -> String {
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
