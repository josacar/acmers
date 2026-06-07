use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.dnspod.com";

pub struct Dpi {
    login_token: String,
}

impl DnsProvider for Dpi {
    fn slug() -> &'static str {
        "dpi"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DP_Id", "DP_Key"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let id = env.get("DP_Id")
            .ok_or_else(|| Error::Config("DP_Id required".into()))?
            .clone();
        let key = env.get("DP_Key")
            .ok_or_else(|| Error::Config("DP_Key required".into()))?
            .clone();
        let login_token = format!("{id},{key}");
        Ok(Box::new(Dpi { login_token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = self.resolve_domain(domain)?;
        let short_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);

        let body = format!(
            "login_token={lt}&format=json&domain_id={did}&sub_domain={sn}&record_type=TXT&record_line=%E9%BB%98%E8%AE%A4&value={val}&ttl=120",
            lt = url_encode(&self.login_token),
            did = domain_id,
            sn = url_encode(short_name),
            val = url_encode(value),
        );
        let resp = http::post(&format!("{BASE_URL}/Record.Create"), body.as_bytes(), "application/x-www-form-urlencoded", &[])
            .map_err(|e| Error::Provider(format!("DNSPod Intl add TXT: {e}")))?;

        if resp.status >= 300 {
            return Err(Error::Provider(format!("DNSPod Intl add TXT: {} {}", resp.status, resp.body)));
        }

        let v: serde_json::Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("DNSPod Intl response: {e}")))?;
        if let Some(err_msg) = v.get("status").and_then(|s| s.get("message")).and_then(|m| m.as_str()) {
            let code = v.get("status").and_then(|s| s.get("code")).and_then(|c| c.as_str()).unwrap_or("");
            if code != "1" {
                return Err(Error::Provider(format!("DNSPod Intl add TXT: {err_msg}")));
            }
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = match self.resolve_domain(domain) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let short_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);

        let record_id = match self.find_record(&domain_id, short_name, value) {
            Ok(Some(id)) => id,
            Ok(None) => return Ok(()),
            Err(_) => return Ok(()),
        };

        let del_url = format!("{BASE_URL}/Record.Remove");
        let del_body = format!(
            "login_token={lt}&format=json&domain_id={did}&record_id={rid}",
            lt = url_encode(&self.login_token),
            did = domain_id,
            rid = record_id,
        );
        let _ = http::post(&del_url, del_body.as_bytes(), "application/x-www-form-urlencoded", &[]);
        Ok(())
    }
}

impl Dpi {
    fn resolve_domain(&self, domain: &str) -> Result<String, Error> {
        let body = format!(
            "login_token={lt}&format=json&type=all",
            lt = url_encode(&self.login_token),
        );
        let resp = http::post(&format!("{BASE_URL}/Domain.List"), body.as_bytes(), "application/x-www-form-urlencoded", &[])
            .map_err(|e| Error::Provider(format!("DNSPod Intl list domains: {e}")))?;

        if resp.status >= 300 {
            return Err(Error::Provider(format!("DNSPod Intl list domains: {} {}", resp.status, resp.body)));
        }

        let v: serde_json::Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("DNSPod Intl domains: {e}")))?;

        if let Some(domains) = v.get("domains").and_then(|d| d.as_array()) {
            for d in domains {
                if let Some(name) = d.get("name").and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        if let Some(id) = d.get("id").and_then(|i| {
                            if i.is_string() {
                                i.as_str().map(|s| s.to_string())
                            } else if i.is_i64() {
                                Some(i.as_i64().unwrap().to_string())
                            } else {
                                None
                            }
                        }) {
                            return Ok(id);
                        }
                    }
                }
            }
        }

        Err(Error::Provider(format!("domain not found: {domain}")))
    }

    fn find_record(&self, domain_id: &str, short_name: &str, value: &str) -> Result<Option<String>, Error> {
        let body = format!(
            "login_token={lt}&format=json&domain_id={did}&sub_domain={sn}&record_type=TXT",
            lt = url_encode(&self.login_token),
            did = domain_id,
            sn = url_encode(short_name),
        );
        let list_url = format!("{BASE_URL}/Record.List");
        let resp = http::post(&list_url, body.as_bytes(), "application/x-www-form-urlencoded", &[])
            .map_err(|e| Error::Provider(format!("DNSPod Intl list records: {e}")))?;

        if resp.status >= 300 {
            return Ok(None);
        }

        let v: serde_json::Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("DNSPod Intl records: {e}")))?;

        if let Some(records) = v.get("records").and_then(|r| r.as_array()) {
            for record in records {
                let val = record.get("value").and_then(|v| v.as_str()).unwrap_or("");
                if val == value {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if i.is_string() {
                            i.as_str().map(|s| s.to_string())
                        } else if i.is_i64() {
                            Some(i.as_i64().unwrap().to_string())
                        } else {
                            None
                        }
                    }) {
                        return Ok(Some(id));
                    }
                }
            }
        }
        Ok(None)
    }
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push_str("%20"),
            _ => {
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0x0f) as usize] as char);
            }
        }
    }
    out
}

const HEX: &[u8] = b"0123456789ABCDEF";
