use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.dns.la/api";

pub struct La {
    id: String,
    sk: String,
}

impl DnsProvider for La {
    fn slug() -> &'static str {
        "la"
    }

    fn env_vars() -> &'static [&'static str] {
        &["LA_Id", "LA_Sk"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let id = env.get("LA_Id")
            .ok_or_else(|| Error::Config("LA_Id required".into()))?
            .clone();
        let sk = env.get("LA_Sk")
            .ok_or_else(|| Error::Config("LA_Sk required".into()))?
            .clone();
        Ok(Box::new(La { id, sk }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = auth_header(&self.id, &self.sk);
        let (domain_id, sub_domain) = self.resolve_root(domain, name)?;

        let body = serde_json::json!({
            "domainId": domain_id,
            "type": 16,
            "host": sub_domain,
            "data": value,
            "ttl": 600,
        });
        let url = format!("{BASE_URL}/record");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("la add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("la add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Provider(format!("la add TXT parse: {e}")))?;
        if v.get("id").is_some() || v.get("msg").and_then(|m| m.as_str()) == Some("\u{4e0e}\u{5df2}\u{6709}\u{8bb0}\u{5f55}\u{51b2}\u{7a81}") {
            return Ok(());
        }
        if v.get("code").and_then(|c| c.as_u64()) == Some(200) {
            return Ok(());
        }
        Err(Error::Provider(format!("la add TXT: {}", resp.body)))
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = auth_header(&self.id, &self.sk);
        let (domain_id, sub_domain) = self.resolve_root(domain, name)?;

        let url = format!("{BASE_URL}/recordList?pageIndex=1&pageSize=10&domainId={}&host={}&type=16&data={}",
            domain_id, sub_domain, value);
        let resp = match http::get(&url, &[("Authorization", &auth)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let record_id = v.get("id").and_then(|i| i.as_str()).map(|s| s.to_string())
            .or_else(|| {
                v.get("data").and_then(|d| d.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|r| r.get("id"))
                    .and_then(|i| {
                        if i.is_string() { i.as_str().map(|s| s.to_string()) }
                        else if i.is_u64() { i.as_u64().map(|n| n.to_string()) }
                        else if i.is_i64() { i.as_i64().map(|n| n.to_string()) }
                        else { None }
                    })
            });
        let record_id = match record_id {
            Some(id) => id,
            None => return Ok(()),
        };

        let del_url = format!("{BASE_URL}/record?id={}", record_id);
        let _ = http::delete(&del_url, &[("Authorization", &auth)]);
        Ok(())
    }
}

impl La {
    fn resolve_root(&self, domain: &str, name: &str) -> Result<(String, String), Error> {
        let auth = auth_header(&self.id, &self.sk);
        let full = format!("{}.{}", name, domain);
        let parts: Vec<&str> = full.split('.').collect();
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            let url = format!("{BASE_URL}/domain?domain={}", h);
            let resp = http::get(&url, &[("Authorization", &auth)])
                .map_err(|e| Error::Provider(format!("la zone resolution: {e}")))?;
            let v: Value = serde_json::from_str(&resp.body)
                .map_err(|e| Error::Provider(format!("la zone parse: {e}")))?;
            if v.get("domain").is_some() {
                if let Some(id) = v.get("id").and_then(|i| {
                    if i.is_string() { i.as_str().map(|s| s.to_string()) }
                    else if i.is_u64() { i.as_u64().map(|n| n.to_string()) }
                    else if i.is_i64() { i.as_i64().map(|n| n.to_string()) }
                    else { None }
                }) {
                    let sub_domain = parts[..i].join(".");
                    return Ok((id, sub_domain));
                }
            }
        }
        Err(Error::Provider("la: could not resolve root domain".into()))
    }
}

fn auth_header(id: &str, sk: &str) -> String {
    let creds = base64::encode_std(format!("{id}:{sk}").as_bytes());
    format!("Basic {creds}")
}
