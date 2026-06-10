use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.reg.ru/api/regru2";

pub struct Regru {
    username: String,
    password: String,
}

impl DnsProvider for Regru {
    fn slug() -> &'static str {
        "regru"
    }

    fn env_vars() -> &'static [&'static str] {
        &["REGRU_Username", "REGRU_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("REGRU_Username")
            .ok_or_else(|| Error::Config("REGRU_Username required".into()))?
            .clone();
        let password = env.get("REGRU_Password")
            .ok_or_else(|| Error::Config("REGRU_Password required".into()))?
            .clone();
        Ok(Box::new(Regru { username, password }))
    }

    fn add_txt(&self, domain: &str, _name: &str, value: &str) -> ProviderResult {
        let fulldomain = domain.to_lowercase();
        let (root_domain, subdomain) = self.get_root(&fulldomain)?;

        let input_json = serde_json::json!({
            "username": self.username,
            "password": self.password,
            "domains": [{"dname": root_domain}],
            "subdomain": subdomain,
            "text": value,
            "output_content_type": "plain"
        });

        let input_data = url_encode(&serde_json::to_string(&input_json).unwrap());
        let body = format!("input_data={}&input_format=json", input_data);

        let resp = http::post(
            &format!("{}/zone/add_txt", BASE_URL),
            body.as_bytes(),
            "application/x-www-form-urlencoded",
            &[],
        ).map_err(|e| Error::Provider(format!("regru API: {e}")))?;

        if resp.body.contains("error") {
            return Err(Error::Provider(format!("regru add_txt error: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, _name: &str, value: &str) -> ProviderResult {
        let fulldomain = domain.to_lowercase();
        let (root_domain, subdomain) = match self.get_root(&fulldomain) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };

        let input_json = serde_json::json!({
            "username": self.username,
            "password": self.password,
            "domains": [{"dname": root_domain}],
            "subdomain": subdomain,
            "content": value,
            "record_type": "TXT",
            "output_content_type": "plain"
        });

        let input_data = url_encode(&serde_json::to_string(&input_json).unwrap());
        let body = format!("input_data={}&input_format=json", input_data);

        let resp = match http::post(
            &format!("{}/zone/remove_record", BASE_URL),
            body.as_bytes(),
            "application/x-www-form-urlencoded",
            &[],
        ) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };

        if resp.body.contains("error") {
            eprintln!("warning: regru remove_txt returned error: {}", resp.body);
            return Ok(());
        }
        Ok(())
    }
}

impl Regru {
    fn get_root(&self, fulldomain: &str) -> Result<(String, String), Error> {
        let body = format!(
            "username={}&password={}&output_format=xml&servtype=domain",
            url_encode(&self.username),
            url_encode(&self.password),
        );

        let resp = http::post(
            &format!("{}/service/get_list", BASE_URL),
            body.as_bytes(),
            "application/x-www-form-urlencoded",
            &[],
        ).map_err(|e| Error::Provider(format!("regru API: {e}")))?;

        if resp.status >= 400 {
            return Err(Error::Provider(format!("regru HTTP {}: {}", resp.status, resp.body)));
        }

        let domains = extract_dnames(&resp.body);

        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 1..parts.len() {
            let candidate = parts[i..].join(".");
            for d in &domains {
                if candidate.ends_with(d.as_str()) || candidate == *d {
                    let subdomain = if i > 0 {
                        parts[..i].join(".")
                    } else {
                        String::new()
                    };
                    return Ok((d.clone(), subdomain));
                }
            }
        }

        Err(Error::Provider(format!("regru: cannot find zone for {fulldomain}")))
    }
}

fn extract_dnames(xml: &str) -> Vec<String> {
    let mut domains = Vec::new();
    let mut search_from = 0;
    while let Some(start) = xml[search_from..].find("dname=\"") {
        let abs_start = search_from + start + 7;
        if let Some(end) = xml[abs_start..].find('"') {
            domains.push(xml[abs_start..abs_start + end].to_string());
            search_from = abs_start + end + 1;
        } else {
            break;
        }
    }
    domains
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => {
                out.push('%');
                out.push('2');
                out.push('0');
            }
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
