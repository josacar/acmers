use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://1984.hosting";

pub struct Hosting1984 {
    username: String,
    password: String,
}

struct Session {
    cookies: String,
    csrf_token: String,
}

impl DnsProvider for Hosting1984 {
    fn slug() -> &'static str {
        "hosting1984"
    }

    fn env_vars() -> &'static [&'static str] {
        &["One984HOSTING_Username", "One984HOSTING_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("One984HOSTING_Username")
            .ok_or_else(|| Error::Config("One984HOSTING_Username required".into()))?
            .clone();
        let password = env.get("One984HOSTING_Password")
            .ok_or_else(|| Error::Config("One984HOSTING_Password required".into()))?
            .clone();
        Ok(Box::new(Hosting1984 { username, password }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let session = self.login()?;
        let (sub_domain, zone) = self.get_root(name, &session)?;
        let encoded_value = url_encode(value);
        let form = format!(
            "entry=new&type=TXT&ttl=900&zone={}&host={}&rdata=%22{}%22",
            url_encode(&zone),
            url_encode(&sub_domain),
            encoded_value,
        );
        let zone_id = self.get_zone_id(&zone, &session)?;
        let referer = format!("{BASE_URL}/domains/{zone_id}");
        let headers: &[(&str, &str)] = &[
            ("Cookie", &session.cookies),
            ("Referer", &referer),
            ("X-CSRFToken", &session.csrf_token),
        ];
        let url = format!("{BASE_URL}/domains/entry/");
        let resp = http::post(&url, form.as_bytes(), "application/x-www-form-urlencoded", headers)
            .map_err(|e| Error::Provider(format!("Hosting1984 add TXT: {e}")))?;
        if resp.body.contains("\"haserrors\": true") || resp.body.contains("\"haserrors\":true") {
            return Err(Error::Provider(format!("Hosting1984 add TXT: haserrors: {}", resp.body)));
        }
        if resp.body.contains("html>") {
            return Err(Error::Provider(format!("Hosting1984 add TXT: got HTML: {}", resp.body)));
        }
        if resp.body.contains("\"auth\": false") || resp.body.contains("\"auth\":false") {
            return Err(Error::Provider("Hosting1984 add TXT: invalid or expired cookie".into()));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let session = match self.login() {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        let (_sub_domain, zone) = match self.get_root(name, &session) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let zone_id = match self.get_zone_id(&zone, &session) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let zone_url = format!("{BASE_URL}/domains/{zone_id}");
        let get_headers: &[(&str, &str)] = &[("Cookie", &session.cookies)];
        let resp = match http::get(&zone_url, get_headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let entry_id = match find_entry_id(&resp.body, value) {
            Some(id) => id,
            None => return Ok(()),
        };
        let del_form = format!("entry={entry_id}");
        let del_url = format!("{BASE_URL}/domains/delentry/");
        let referer = format!("{BASE_URL}/domains/{zone_id}");
        let post_headers: &[(&str, &str)] = &[
            ("Cookie", &session.cookies),
            ("Referer", &referer),
            ("X-CSRFToken", &session.csrf_token),
        ];
        match http::post(&del_url, del_form.as_bytes(), "application/x-www-form-urlencoded", post_headers) {
            Ok(resp) => {
                if !resp.body.contains("\"ok\": true") && !resp.body.contains("\"ok\":true") {
                    eprintln!("warning: Hosting1984 delete TXT: {}", resp.body);
                }
            }
            Err(e) => {
                eprintln!("warning: Hosting1984 delete TXT: {e}");
            }
        }
        Ok(())
    }
}

impl Hosting1984 {
    fn login(&self) -> Result<Session, Error> {
        let login_url = format!("{BASE_URL}/accounts/login/");
        let resp = http::get(&login_url, &[])
            .map_err(|e| Error::Provider(format!("Hosting1984 login page: {e}")))?;
        let initial_cookies = extract_cookies(&resp);
        let csrf_cookie = extract_cookie_value(&initial_cookies, "csrftoken")
            .ok_or_else(|| Error::Provider("Hosting1984: no csrftoken cookie".into()))?;
        let session_cookie = extract_cookie_value(&initial_cookies, "cookie1984nammnamm")
            .ok_or_else(|| Error::Provider("Hosting1984: no session cookie".into()))?;
        let cookies = format!("csrftoken={csrf_cookie}; cookie1984nammnamm={session_cookie}");
        let csrf_header = csrf_cookie.clone();
        let form = format!(
            "username={}&password={}&otpkey=",
            url_encode(&self.username),
            url_encode(&self.password),
        );
        let headers: &[(&str, &str)] = &[
            ("Cookie", &cookies),
            ("Referer", &login_url),
            ("X-CSRFToken", &csrf_header),
        ];
        let auth_url = format!("{BASE_URL}/api/auth/");
        let resp = http::post(&auth_url, form.as_bytes(), "application/x-www-form-urlencoded", headers)
            .map_err(|e| Error::Provider(format!("Hosting1984 login: {e}")))?;
        if !resp.body.contains("\"loggedin\": true") && !resp.body.contains("\"loggedin\":true") {
            return Err(Error::Provider(format!("Hosting1984 login failed for user {}", self.username)));
        }
        let new_cookies = extract_cookies(&resp);
        let final_csrf = extract_cookie_value(&new_cookies, "csrftoken")
            .unwrap_or(csrf_cookie);
        let final_session = extract_cookie_value(&new_cookies, "cookie1984nammnamm")
            .unwrap_or(session_cookie);
        let final_cookies = format!("csrftoken={final_csrf}; cookie1984nammnamm={final_session}");
        Ok(Session {
            cookies: final_cookies,
            csrf_token: final_csrf,
        })
    }

    fn get_root(&self, fulldomain: &str, session: &Session) -> Result<(String, String), Error> {
        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                continue;
            }
            let url = format!("{BASE_URL}/domains/zonestatus/{h}/?cached=no");
            let headers: &[(&str, &str)] = &[("Cookie", &session.cookies)];
            let resp = match http::get(&url, headers) {
                Ok(r) => r,
                Err(_) => continue,
            };
            if resp.body.contains("\"ok\": true") || resp.body.contains("\"ok\":true") {
                let sub_domain = parts[..=i].join(".");
                return Ok((sub_domain, h));
            }
        }
        Err(Error::Provider(format!("Hosting1984: could not find zone for {fulldomain}")))
    }

    fn get_zone_id(&self, zone: &str, session: &Session) -> Result<String, Error> {
        let url = format!("{BASE_URL}/domains");
        let headers: &[(&str, &str)] = &[("Cookie", &session.cookies)];
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("Hosting1984 get zone_id: {e}")))?;
        find_zone_id(&resp.body, zone)
            .ok_or_else(|| Error::Provider(format!("Hosting1984: could not find zone_id for {zone}")))
    }
}

fn extract_cookies(resp: &http::Response) -> String {
    let mut cookies = Vec::new();
    if let Some(raw) = resp.headers.get("set-cookie") {
        for part in raw.split(',') {
            let part = part.trim();
            if let Some(semi) = part.find(';') {
                if part[..semi].contains('=') {
                    cookies.push(part[..semi].trim().to_string());
                }
            } else if part.contains('=') {
                cookies.push(part.trim().to_string());
            }
        }
    }
    cookies.join("; ")
}

fn extract_cookie_value(cookies: &str, name: &str) -> Option<String> {
    for part in cookies.split(';') {
        let part = part.trim();
        if let Some(eq) = part.find('=') {
            if part[..eq].trim() == name {
                return Some(part[eq + 1..].trim().to_string());
            }
        }
    }
    None
}

fn find_zone_id(html: &str, zone: &str) -> Option<String> {
    let marker = "zone/";
    let mut search = html;
    loop {
        let pos = search.find(marker)?;
        let after = &search[pos + marker.len()..];
        let end = after.find(|c: char| !c.is_ascii_digit()).unwrap_or(after.len());
        if end == 0 {
            search = after;
            continue;
        }
        let id = &after[..end];
        let context_start = pos.saturating_sub(200);
        let context_end = (pos + marker.len() + end + 200).min(html.len());
        let context = &html[context_start..context_end];
        if context.contains(zone) {
            return Some(id.to_string());
        }
        search = after;
    }
}

fn find_entry_id(html: &str, value: &str) -> Option<String> {
    let marker = "entry_";
    let mut search = html;
    loop {
        let pos = search.find(marker)?;
        let after = &search[pos + marker.len()..];
        let end = after.find(|c: char| !c.is_ascii_digit()).unwrap_or(after.len());
        if end == 0 {
            search = after;
            continue;
        }
        let id = &after[..end];
        let line_start = search[..pos].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let line_end = search[pos..].find('\n').map(|p| pos + p).unwrap_or(search.len());
        let line = &search[line_start..line_end];
        if line.contains(value) {
            return Some(id.to_string());
        }
        search = after;
    }
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
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
