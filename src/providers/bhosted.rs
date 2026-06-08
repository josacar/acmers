use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://webservices.bhosted.com/dns";

pub struct Bhosted {
    username: String,
    password: String,
    id_cache: Mutex<HashMap<String, String>>,
}

impl DnsProvider for Bhosted {
    fn slug() -> &'static str {
        "bhosted"
    }

    fn env_vars() -> &'static [&'static str] {
        &["BHOSTED_Username", "BHOSTED_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("BHOSTED_Username")
            .ok_or_else(|| Error::Config("BHOSTED_Username required".into()))?
            .clone();
        let password = env.get("BHOSTED_Password")
            .ok_or_else(|| Error::Config("BHOSTED_Password required".into()))?
            .clone();
        Ok(Box::new(Bhosted {
            username,
            password,
            id_cache: Mutex::new(HashMap::new()),
        }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let (sld, tld, rec_name) = self.parse_domain(domain, name)?;
        let form = format!(
            "user={}&password={}&tld={}&sld={}&type=TXT&name={}&content={}&ttl=300",
            url_encode(&self.username),
            url_encode(&self.password),
            url_encode(&tld),
            url_encode(&sld),
            url_encode(&rec_name),
            url_encode(value),
        );
        let url = format!("{BASE_URL}/addrecord");
        let resp = http::post(&url, form.as_bytes(), "application/x-www-form-urlencoded", &[])
            .map_err(|e| Error::Provider(format!("bhosted add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("bhosted add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        if bhosted_response_has_error(&resp.body) {
            return Err(Error::Provider(format!("bhosted add TXT: {}", resp.body)));
        }
        if let Some(id) = xml_value(&resp.body, "id") {
            let cache_key = format!("{domain}|{name}|{value}");
            self.id_cache.lock().unwrap().insert(cache_key, id);
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let (sld, tld, _rec_name) = self.parse_domain(domain, name)?;
        let cache_key = format!("{domain}|{name}|{value}");
        let rec_id = {
            let cache = self.id_cache.lock().unwrap();
            cache.get(&cache_key).cloned()
        };
        let rec_id = match rec_id {
            Some(id) => id,
            None => return Ok(()),
        };
        let form = format!(
            "user={}&password={}&tld={}&sld={}&id={}",
            url_encode(&self.username),
            url_encode(&self.password),
            url_encode(&tld),
            url_encode(&sld),
            url_encode(&rec_id),
        );
        let url = format!("{BASE_URL}/delrecord");
        let resp = http::post(&url, form.as_bytes(), "application/x-www-form-urlencoded", &[])
            .map_err(|e| Error::Provider(format!("bhosted del TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("bhosted del TXT: HTTP {} {}", resp.status, resp.body)));
        }
        if bhosted_response_has_error(&resp.body) {
            return Err(Error::Provider(format!("bhosted del TXT: {}", resp.body)));
        }
        self.id_cache.lock().unwrap().remove(&cache_key);
        Ok(())
    }
}

impl Bhosted {
    fn parse_domain(&self, domain: &str, name: &str) -> Result<(String, String, String), Error> {
        let parts: Vec<&str> = domain.rsplitn(3, '.').collect();
        if parts.len() < 2 {
            return Err(Error::Provider(format!("bhosted: cannot parse domain: {domain}")));
        }
        let tld = parts[0];
        let sld = parts[1];
        let zone = format!("{sld}.{tld}");
        let rec_name = if name == zone {
            "@".to_string()
        } else if let Some(prefix) = name.strip_suffix(&format!(".{zone}")) {
            if prefix.is_empty() { "@".to_string() } else { prefix.to_string() }
        } else {
            name.to_string()
        };
        Ok((sld.to_string(), tld.to_string(), rec_name))
    }
}

fn xml_value(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)?;
    let content_start = start + open.len();
    let end = xml[content_start..].find(&close)?;
    Some(xml[content_start..content_start + end].to_string())
}

fn bhosted_response_has_error(body: &str) -> bool {
    if body.is_empty() {
        return true;
    }
    if body.contains("<response>") {
        let errors = xml_value(body, "errors").unwrap_or_default();
        let done = xml_value(body, "done").unwrap_or_default();
        return !(errors == "0" && done == "true");
    }
    let lower = body.to_lowercase();
    lower.contains("error")
        || lower.contains("fout")
        || lower.contains("invalid")
        || lower.contains("failed")
        || lower.contains("denied")
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
    if n < 10 { (b'0' + n) as char } else { (b'a' + n - 10) as char }
}
