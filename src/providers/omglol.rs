use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.omg.lol";

pub struct Omglol {
    api_key: String,
    address: String,
}

impl DnsProvider for Omglol {
    fn slug() -> &'static str {
        "omglol"
    }

    fn env_vars() -> &'static [&'static str] {
        &["OMG_ApiKey", "OMG_Address"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("OMG_ApiKey")
            .ok_or_else(|| Error::Config("OMG_ApiKey required".into()))?
            .clone();
        let address = env.get("OMG_Address")
            .ok_or_else(|| Error::Config("OMG_Address required".into()))?
            .clone()
            .trim_start_matches('@')
            .to_string();
        Ok(Box::new(Omglol { api_key, address }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = self.auth_header();
        let dns_name = self.dns_record_name(name, domain);

        let body = serde_json::json!({
            "type": "TXT",
            "name": dns_name,
            "data": value,
            "ttl": 30,
        });
        let url = format!("{BASE_URL}/address/{}/dns", self.address);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("omglol add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("omglol add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Provider(format!("omglol add TXT: parse: {e}")))?;
        if v.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
            return Ok(());
        }
        let msg = v.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error");
        Err(Error::Provider(format!("omglol add TXT: {msg}")))
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = self.auth_header();
        let dns_name = self.dns_record_name(name, domain);

        let list_url = format!("{BASE_URL}/address/{}/dns", self.address);
        let resp = match http::get(&list_url, &[("Authorization", &auth)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records = v.get("response").and_then(|r| r.as_array())
            .or_else(|| v.as_array());
        if let Some(records) = records {
            for record in records {
                let rtype = record.get("type").and_then(|t| t.as_str()).unwrap_or("");
                let rname = record.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let rdata = record.get("data").and_then(|d| d.as_str()).unwrap_or("");
                let expected_name = format!("{}.{}", dns_name, self.address);
                if rtype == "TXT" && rname == expected_name && rdata == value {
                    if let Some(id) = record_id(record) {
                        let del_url = format!("{BASE_URL}/address/{}/dns/{}", self.address, id);
                        let _ = http::delete(&del_url, &[("Authorization", &auth)]);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Omglol {
    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    fn dns_record_name(&self, name: &str, domain: &str) -> String {
        let fqdn = if name.ends_with(domain) {
            name.to_string()
        } else {
            format!("{}.{}", name, domain)
        };
        let stripped = fqdn.strip_suffix(".omg.lol").unwrap_or(&fqdn);
        let suffix = format!(".{}", self.address);
        stripped.strip_suffix(&suffix).unwrap_or(stripped).to_string()
    }
}

fn record_id(v: &Value) -> Option<String> {
    v.get("id").and_then(|i| {
        if i.is_string() { i.as_str().map(|s| s.to_string()) }
        else if i.is_u64() { i.as_u64().map(|n| n.to_string()) }
        else if i.is_i64() { i.as_i64().map(|n| n.to_string()) }
        else { None }
    })
}
