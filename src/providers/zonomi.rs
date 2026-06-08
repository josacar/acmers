use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://zonomi.com/app/dns/dyndns.jsp";

pub struct Zonomi {
    api_key: String,
}

impl DnsProvider for Zonomi {
    fn slug() -> &'static str {
        "zonomi"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ZM_Key"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("ZM_Key")
            .ok_or_else(|| Error::Config("ZM_Key required".into()))?
            .clone();
        Ok(Box::new(Zonomi { api_key }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let enc_key = url_encode(&self.api_key);
        let enc_name = url_encode(name);
        let query_url = format!("{BASE_URL}?api_key={enc_key}&action=QUERY&name={enc_name}");
        let resp = http::get(&query_url, &[])
            .map_err(|e| Error::Provider(format!("zonomi query TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("zonomi query TXT: HTTP {} {}", resp.status, resp.body)));
        }
        if !resp.body.contains("<is_ok>OK:") {
            return Err(Error::Provider(format!("zonomi query TXT: {}", resp.body)));
        }
        let existing = extract_txt_values(&resp.body);
        let enc_value = url_encode(value);
        let set_url = if existing.is_empty() {
            format!("{BASE_URL}?api_key={enc_key}&action=SET&type=TXT&name={enc_name}&value={enc_value}")
        } else {
            let mut qstr = format!("action[1]=SET&type[1]=TXT&name[1]={enc_name}&value[1]={enc_value}");
            for (i, v) in existing.iter().enumerate() {
                let idx = i + 2;
                let ev = url_encode(v);
                qstr.push_str(&format!("&action[{idx}]=SET&type[{idx}]=TXT&name[{idx}]={enc_name}&value[{idx}]={ev}"));
            }
            format!("{BASE_URL}?api_key={enc_key}&{qstr}")
        };
        let resp = http::get(&set_url, &[])
            .map_err(|e| Error::Provider(format!("zonomi add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("zonomi add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        if !resp.body.contains("<is_ok>OK:") {
            return Err(Error::Provider(format!("zonomi add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, _value: &str) -> ProviderResult {
        let enc_name = url_encode(name);
        let enc_key = url_encode(&self.api_key);
        let url = format!("{BASE_URL}?api_key={enc_key}&action=DELETE&type=TXT&name={enc_name}");
        let _ = http::get(&url, &[]);
        Ok(())
    }
}

fn extract_txt_values(xml: &str) -> Vec<String> {
    let mut values = Vec::new();
    for segment in xml.split('<') {
        if !segment.starts_with("record") {
            continue;
        }
        if !segment.contains("type=\"TXT\"") {
            continue;
        }
        if let Some(val) = extract_attr(segment, "value") {
            values.push(val);
        }
    }
    values
}

fn extract_attr(segment: &str, attr: &str) -> Option<String> {
    let pattern = format!("{attr}=\"");
    let start = segment.find(&pattern)? + pattern.len();
    let rest = &segment[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
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
