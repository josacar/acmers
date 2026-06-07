use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Bookmyname {
    username: String,
    password: String,
}

impl DnsProvider for Bookmyname {
    fn slug() -> &'static str {
        "bookmyname"
    }

    fn env_vars() -> &'static [&'static str] {
        &["BOOKMYNAME_Username", "BOOKMYNAME_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("BOOKMYNAME_Username")
            .ok_or_else(|| Error::Config("BOOKMYNAME_Username required".into()))?.clone();
        let password = env.get("BOOKMYNAME_Password")
            .ok_or_else(|| Error::Config("BOOKMYNAME_Password required".into()))?.clone();
        Ok(Box::new(Bookmyname { username, password }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let _ = self.resolve_zone(domain)?;
        let url = "https://api.bookmyname.com/domain/updaterecord/dns";
        let body = serde_json::json!({
            "apiuser": self.username,
            "apipassword": self.password,
            "domain": domain,
            "subdomain": name,
            "type": "TXT",
            "content": value,
            "ttl": 120,
        });
        let resp = http::post(url, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("bookmyname add TXT: {e}")))?;
        if resp.status != 200 {
            return Err(Error::Provider(format!("bookmyname add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let _ = self.resolve_zone(domain)?;
        let url = "https://api.bookmyname.com/domain/updaterecord/dns";
        let body = serde_json::json!({
            "apiuser": self.username,
            "apipassword": self.password,
            "domain": domain,
            "subdomain": name,
            "type": "TXT",
            "content": "",
            "ttl": 120,
        });
        http::post(url, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("bookmyname delete TXT: {e}")))?;
        Ok(())
    }
}

impl Bookmyname {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let url = "https://api.bookmyname.com/domain/list";
        let body = serde_json::json!({
            "apiuser": self.username,
            "apipassword": self.password,
        });
        let resp = http::post(url, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("bookmyname list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("bookmyname domains: {e}")))?;
        if let Some(arr) = v.as_array() {
            for d in arr {
                if let Some(nm) = d.as_str() {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        return Ok(nm.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
