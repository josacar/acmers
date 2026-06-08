use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Leaseweb {
    api_key: String,
}

impl DnsProvider for Leaseweb {
    fn slug() -> &'static str {
        "leaseweb"
    }

    fn env_vars() -> &'static [&'static str] {
        &["LSW_Key"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("LSW_Key")
            .ok_or_else(|| Error::Config("LSW_Key required".into()))?.clone();
        Ok(Box::new(Leaseweb { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("X-Lsw-Auth", &self.api_key)];
        let zone = self.resolve_domain(domain, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "name": format!("{}.", name),
            "type": "TXT",
            "content": [value],
            "ttl": 60,
        })).unwrap();
        let url = format!("https://api.leaseweb.com/hosting/v2/domains/{}/resourceRecordSets", zone);
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("Leaseweb add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Leaseweb add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("X-Lsw-Auth", &self.api_key)];
        let zone = match self.resolve_domain(domain, headers) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let url = format!("https://api.leaseweb.com/hosting/v2/domains/{}/resourceRecordSets/{}/TXT", zone, name);
        let _ = http::delete(&url, headers);
        Ok(())
    }
}

impl Leaseweb {
    fn resolve_domain(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let parts: Vec<&str> = domain.split('.').collect();
        if parts.len() < 3 {
            return Err(Error::Provider(format!("zone not found for {domain}")));
        }
        for i in (1..parts.len() - 1).rev() {
            let candidate = parts[i..].join(".");
            let url = format!("https://api.leaseweb.com/hosting/v2/domains/{}", candidate);
            if let Ok(resp) = http::get(&url, headers) {
                if resp.status == 200 {
                    return Ok(candidate);
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
