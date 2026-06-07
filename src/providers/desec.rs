use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Desec {
    token: String,
}

impl DnsProvider for Desec {
    fn slug() -> &'static str {
        "desec"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DESEC_Token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("DESEC_Token")
            .ok_or_else(|| Error::Config("DESEC_Token required".into()))?.clone();
        Ok(Box::new(Desec { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let _ = self.resolve_zone(domain)?;
        let url = format!("https://desec.io/api/v1/domains/{domain}/rrsets/");
        let body = serde_json::json!({
            "subname": name,
            "type": "TXT",
            "ttl": 120,
            "records": [format!("\"{value}\"")],
        });
        let auth = format!("Token {}", self.token);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("desec add TXT: {e}")))?;
        if resp.status >= 300 {
            return Err(Error::Provider(format!("desec add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let _ = self.resolve_zone(domain)?;
        let url = format!("https://desec.io/api/v1/domains/{domain}/rrsets/{name}/TXT/");
        let body = serde_json::json!({"records": []});
        let auth = format!("Token {}", self.token);
        http::patch(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("desec delete TXT: {e}")))?;
        Ok(())
    }
}

impl Desec {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let auth = format!("Token {}", self.token);
        let resp = http::get("https://desec.io/api/v1/domains/", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("desec list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("desec domains: {e}")))?;
        if let Some(arr) = v.as_array() {
            for d in arr {
                if let Some(nm) = d.get("name").and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        return Ok(nm.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
