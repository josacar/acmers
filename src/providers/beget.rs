use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::json as j;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.beget.com/api";

pub struct Beget {
    username: String,
    password: String,
}

impl DnsProvider for Beget {
    fn slug() -> &'static str {
        "beget"
    }

    fn env_vars() -> &'static [&'static str] {
        &["Beget_Username", "Beget_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("Beget_Username")
            .ok_or_else(|| Error::Config("Beget_Username required".into()))?
            .clone();
        let password = env.get("Beget_Password")
            .ok_or_else(|| Error::Config("Beget_Password required".into()))?
            .clone();
        Ok(Box::new(Beget { username, password }))
    }

    fn add_txt(&self, domain: &str, _name: &str, value: &str) -> ProviderResult {
        let fulldomain = domain.to_lowercase();

        self.prepare_subdomain(&fulldomain)?;

        let resp = self.api_call("dns/getData", &format!("{{\"fqdn\":\"{fulldomain}\"}}"))?;
        let result = j::get_value_required(&resp, &["answer", "result"])?;

        let mut records = serde_json::Map::new();
        for rtype in &["A", "AAAA", "CAA", "MX", "SRV", "TXT"] {
            if let Some(arr) = result.get(*rtype) {
                records.insert(rtype.to_string(), arr.clone());
            } else {
                records.insert(rtype.to_string(), serde_json::json!([]));
            }
        }

        let txt_entry = serde_json::json!({"ttl": 600, "txtdata": value});
        if let Some(serde_json::Value::Array(arr)) = records.get_mut("TXT") {
            arr.push(txt_entry);
        }

        let body = serde_json::json!({
            "fqdn": fulldomain,
            "records": records,
        });
        let resp = self.api_call("dns/changeRecords", &serde_json::to_string(&body).unwrap())?;
        self.check_ok(&resp)
    }

    fn remove_txt(&self, domain: &str, _name: &str, value: &str) -> ProviderResult {
        let fulldomain = domain.to_lowercase();

        let resp = match self.api_call("dns/getData", &format!("{{\"fqdn\":\"{fulldomain}\"}}")) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let result = match j::get_value(&resp, &["answer", "result"]) {
            Some(r) => r,
            None => return Ok(()),
        };

        let mut records = serde_json::Map::new();
        for rtype in &["A", "AAAA", "CAA", "MX", "SRV", "TXT"] {
            if let Some(arr) = result.get(*rtype) {
                records.insert(rtype.to_string(), arr.clone());
            } else {
                records.insert(rtype.to_string(), serde_json::json!([]));
            }
        }

        if let Some(serde_json::Value::Array(arr)) = records.get_mut("TXT") {
            arr.retain(|r| {
                r.get("txtdata").and_then(|v| v.as_str()) != Some(value)
            });
        }

        let body = serde_json::json!({
            "fqdn": fulldomain,
            "records": records,
        });
        let resp = self.api_call("dns/changeRecords", &serde_json::to_string(&body).unwrap())?;
        self.check_ok(&resp)
    }
}

impl Beget {
    fn api_call(&self, endpoint: &str, input_data: &str) -> Result<serde_json::Value, Error> {
        let url = format!(
            "{BASE_URL}/{endpoint}?login={}&passwd={}&input_format=json&output_format=json&input_data={}",
            url_encode(&self.username),
            url_encode(&self.password),
            url_encode(input_data),
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("beget API: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("beget HTTP {}: {}", resp.status, resp.body)));
        }
        serde_json::from_str(&resp.body)
            .map_err(|e| Error::Provider(format!("beget JSON: {e}")))
    }

    fn check_ok(&self, resp: &serde_json::Value) -> ProviderResult {
        let status = j::get_string(resp, &["status"]).unwrap_or("");
        let answer_status = j::get_string(resp, &["answer", "status"]).unwrap_or("");
        if status == "success" && answer_status == "success" {
            Ok(())
        } else {
            Err(Error::Provider(format!("beget API error: {}", resp)))
        }
    }

    fn get_root(&self, fulldomain: &str) -> Result<(u64, String, String), Error> {
        let resp = self.api_call("domain/getList", "")?;
        self.check_ok(&resp)?;

        let domains = j::get_array(&resp, &["answer", "result"])
            .ok_or_else(|| Error::Provider("beget: missing domain list".into()))?;

        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 1..parts.len() {
            let candidate = parts[i..].join(".");
            for d in domains {
                let fqdn = j::get_string(d, &["fqdn"]).unwrap_or("");
                if fqdn == candidate {
                    let id = d.get("id").and_then(|v| v.as_u64())
                        .ok_or_else(|| Error::Provider("beget: missing domain id".into()))?;
                    let sub_domain = if i > 0 {
                        parts[..i].join(".")
                    } else {
                        String::new()
                    };
                    return Ok((id, sub_domain, candidate));
                }
            }
        }
        Err(Error::Provider(format!("beget: cannot find zone for {fulldomain}")))
    }

    fn prepare_subdomain(&self, fulldomain: &str) -> ProviderResult {
        let (domain_id, sub_domain, _domain) = self.get_root(fulldomain)?;

        if sub_domain.is_empty() {
            return Ok(());
        }

        let resp = self.api_call("domain/getSubdomainList", "")?;
        self.check_ok(&resp)?;

        if let Some(subdomains) = j::get_array(&resp, &["answer", "result"]) {
            for sub in subdomains {
                if j::get_string(sub, &["fqdn"]).unwrap_or("") == fulldomain {
                    return Ok(());
                }
            }
        }

        let data = format!("{{\"subdomain\":\"{sub_domain}\",\"domain_id\":{domain_id}}}");
        let resp = self.api_call("domain/addSubdomainVirtual", &data)?;
        self.check_ok(&resp)?;

        let cleanup = format!("{{\"fqdn\":\"{fulldomain}\",\"records\":{{}}}}");
        let _ = self.api_call("dns/changeRecords", &cleanup);
        let cleanup_www = format!("{{\"fqdn\":\"www.{fulldomain}\",\"records\":{{}}}}");
        let _ = self.api_call("dns/changeRecords", &cleanup_www);

        Ok(())
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
