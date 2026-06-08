use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://www.geoscaling.com/dns2";

pub struct Geoscaling {
    username: String,
    password: String,
}

impl DnsProvider for Geoscaling {
    fn slug() -> &'static str {
        "geoscaling"
    }

    fn env_vars() -> &'static [&'static str] {
        &["GEOSCALING_Username", "GEOSCALING_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("GEOSCALING_Username")
            .ok_or_else(|| Error::Config("GEOSCALING_Username required".into()))?
            .clone();
        let password = env.get("GEOSCALING_Password")
            .ok_or_else(|| Error::Config("GEOSCALING_Password required".into()))?
            .clone();
        Ok(Box::new(Geoscaling { username, password }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let cookie = self.login()?;
        let (zone_id, zone_name) = self.find_zone(domain, &cookie)?;
        let headers: &[(&str, &str)] = &[("Cookie", &cookie)];

        let prefix = name.strip_suffix(&format!(".{zone_name}"))
            .unwrap_or(name);

        let body = format!(
            "id={}&name={}&type=TXT&content={}&ttl=300&prio=0",
            zone_id,
            url_encode(prefix),
            url_encode(value),
        );
        let url = format!("{BASE_URL}/ajax/add_record.php");
        let resp = http::post(&url, body.as_bytes(), "application/x-www-form-urlencoded", headers)
            .map_err(|e| Error::Provider(format!("geoscaling add TXT: {e}")))?;
        if resp.status >= 400 {
            let _ = self.logout(&cookie);
            return Err(Error::Provider(format!("geoscaling add TXT: HTTP {}", resp.status)));
        }
        let _ = self.logout(&cookie);
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let cookie = match self.login() {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };
        let (zone_id, _zone_name) = match self.find_zone(domain, &cookie) {
            Ok(z) => z,
            Err(_) => { let _ = self.logout(&cookie); return Ok(()); }
        };
        let headers: &[(&str, &str)] = &[("Cookie", &cookie)];

        let page_url = format!("{BASE_URL}/index.php?module=domain&id={zone_id}");
        let resp = match http::get(&page_url, headers) {
            Ok(r) => r,
            Err(_) => { let _ = self.logout(&cookie); return Ok(()); }
        };

        let flat = resp.body.replace('\n', "");
        let table = match extract_between(&flat, "<div class=\"box\"><div class=\"boxtitle\">Basic Records</div><div class=\"boxtext\"><table", "</table>") {
            Some(t) => format!("<table{t}</table>"),
            None => { let _ = self.logout(&cookie); return Ok(()); }
        };

        let mut found_id = None;
        let mut pos = 0;
        let mut record_ids: Vec<String> = Vec::new();
        let mut record_names: Vec<String> = Vec::new();
        let mut record_types: Vec<String> = Vec::new();
        let mut record_values: Vec<String> = Vec::new();

        loop {
            match table[pos..].find("id=\"") {
                None => break,
                Some(idx) => {
                    let start = pos + idx + 4;
                    if let Some(end_quote) = table[start..].find('"') {
                        let full_id = &table[start..start + end_quote];
                        if let Some(dot_pos) = full_id.rfind('.') {
                            let rid = &full_id[..dot_pos];
                            let field = &full_id[dot_pos + 1..];
                            let after_quote = start + end_quote + 1;
                            if let Some(gt) = table[after_quote..].find('>') {
                                let val_start = after_quote + gt + 1;
                                if let Some(td_end) = table[val_start..].find("</td>") {
                                    let val = &table[val_start..val_start + td_end];
                                    match field {
                                        "name" => {
                                            ensure_vec(&mut record_ids, rid);
                                            ensure_vec(&mut record_names, val);
                                        }
                                        "type" => {
                                            ensure_vec(&mut record_types, val);
                                        }
                                        "content" => {
                                            ensure_vec(&mut record_values, val);
                                        }
                                        _ => {}
                                    }
                                    pos = val_start + td_end + 5;
                                    continue;
                                }
                            }
                        }
                        pos = start + end_quote + 1;
                    } else {
                        break;
                    }
                }
            }
        }

        for i in 0..record_values.len() {
            if record_values[i] == value
                && i < record_types.len() && record_types[i] == "TXT"
                && i < record_names.len() && record_names[i] == name
                && i < record_ids.len()
            {
                found_id = Some(record_ids[i].clone());
                break;
            }
        }

        let found_id = match found_id {
            Some(id) => id,
            None => { let _ = self.logout(&cookie); return Ok(()); }
        };

        let body = format!("id={zone_id}&record_id={found_id}");
        let url = format!("{BASE_URL}/ajax/delete_record.php");
        let _ = http::post(&url, body.as_bytes(), "application/x-www-form-urlencoded", headers);
        let _ = self.logout(&cookie);
        Ok(())
    }
}

impl Geoscaling {
    fn login(&self) -> Result<String, Error> {
        let body = format!(
            "username={}&password={}",
            url_encode(&self.username),
            url_encode(&self.password),
        );
        let url = format!("{BASE_URL}/index.php?module=auth");
        let resp = http::post(&url, body.as_bytes(), "application/x-www-form-urlencoded", &[])
            .map_err(|e| Error::Provider(format!("geoscaling login: {e}")))?;

        let set_cookie = resp.headers.get("set-cookie")
            .cloned()
            .unwrap_or_default();

        let phpsessid = extract_phpsessid(&set_cookie)
            .ok_or_else(|| Error::Provider("geoscaling login: no PHPSESSID in response".into()))?;

        Ok(phpsessid)
    }

    fn logout(&self, cookie: &str) -> Result<(), Error> {
        let headers: &[(&str, &str)] = &[("Cookie", cookie)];
        let url = format!("{BASE_URL}/index.php?module=auth");
        let _ = http::get(&url, headers);
        Ok(())
    }

    fn find_zone(&self, domain: &str, cookie: &str) -> Result<(String, String), Error> {
        let headers: &[(&str, &str)] = &[("Cookie", cookie)];
        let url = format!("{BASE_URL}/index.php?module=domains");
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("geoscaling list zones: {e}")))?;

        let flat = resp.body.replace('\n', "");
        let table = extract_between(&flat,
            "<div class=\"box\"><div class=\"boxtitle\">Your domains</div><div class=\"boxtext\"><table",
            "</table>")
            .map(|t| format!("<table{t}</table>"))
            .ok_or_else(|| Error::Provider("geoscaling: could not find domains table".into()))?;

        let zone_names = extract_all(&table, "<b>", "</b>");
        let zone_ids = extract_zone_ids(&table);

        if zone_names.is_empty() || zone_ids.is_empty() {
            return Err(Error::Provider("geoscaling: could not parse zone names or IDs".into()));
        }

        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len() {
            let attempt = parts[i..].join(".");
            if let Some(idx) = zone_names.iter().position(|n| n == &attempt) {
                if idx < zone_ids.len() {
                    return Ok((zone_ids[idx].clone(), attempt));
                }
            }
        }

        Err(Error::Provider(format!("geoscaling: no zone found for {domain}")))
    }
}

fn extract_phpsessid(cookie_header: &str) -> Option<String> {
    for part in cookie_header.split(',') {
        let part = part.trim();
        for segment in part.split(';') {
            let segment = segment.trim();
            if segment.starts_with("PHPSESSID=") {
                return Some(segment.to_string());
            }
        }
    }
    None
}

fn extract_between<'a>(text: &'a str, start_marker: &str, end_marker: &str) -> Option<&'a str> {
    let start = text.find(start_marker)? + start_marker.len();
    let end = text[start..].find(end_marker)?;
    Some(&text[start..start + end])
}

fn extract_all(text: &str, open: &str, close: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut pos = 0;
    while pos < text.len() {
        if let Some(start_idx) = text[pos..].find(open) {
            let val_start = pos + start_idx + open.len();
            if let Some(end_idx) = text[val_start..].find(close) {
                results.push(text[val_start..val_start + end_idx].to_string());
                pos = val_start + end_idx + close.len();
            } else {
                break;
            }
        } else {
            break;
        }
    }
    results
}

fn extract_zone_ids(table: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let marker = "module=domain&id=";
    let mut pos = 0;
    while pos < table.len() {
        if let Some(idx) = table[pos..].find(marker) {
            let id_start = pos + idx + marker.len();
            let mut id_end = id_start;
            for ch in table[id_start..].chars() {
                if ch.is_ascii_digit() {
                    id_end += ch.len_utf8();
                } else {
                    break;
                }
            }
            if id_end > id_start {
                ids.push(table[id_start..id_end].to_string());
            }
            pos = id_end;
        } else {
            break;
        }
    }
    ids
}

fn ensure_vec(vec: &mut Vec<String>, val: &str) {
    let trimmed = val.trim();
    vec.push(trimmed.to_string());
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
    if n < 10 { (b'0' + n) as char } else { (b'A' + n - 10) as char }
}
