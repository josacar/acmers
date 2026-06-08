use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.nederhost.nl/dns/v1";

pub struct Nederhost {
    key: String,
}

impl DnsProvider for Nederhost {
    fn slug() -> &'static str {
        "nederhost"
    }

    fn env_vars() -> &'static [&'static str] {
        &["NederHost_Key"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let key = env.get("NederHost_Key")
            .ok_or_else(|| Error::Config("NederHost_Key required".into()))?
            .clone();
        Ok(Box::new(Nederhost { key }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.key);
        let (zone, _sub) = self.resolve_zone(name)?;

        let body = serde_json::json!([{"content": value, "ttl": 60}]);
        let url = format!("{BASE_URL}/zones/{zone}/records/{name}/TXT");
        let resp = http::patch(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("nederhost add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("nederhost add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.key);
        let (zone, _sub) = self.resolve_zone(name)?;

        let url = format!("{BASE_URL}/zones/{zone}/records/{name}/TXT?content={value}");
        match http::delete(&url, &[("Authorization", &auth)]) {
            Ok(_) => Ok(()),
            Err(e) => {
                eprintln!("warning: nederhost cleanup failed: {e}");
                Ok(())
            }
        }
    }
}

impl Nederhost {
    fn resolve_zone(&self, fulldomain: &str) -> Result<(String, String), Error> {
        let auth = format!("Bearer {}", self.key);
        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 1..parts.len() {
            let domain = parts[i..].join(".");
            let url = format!("{BASE_URL}/zones/{domain}");
            match http::get(&url, &[("Authorization", &auth)]) {
                Ok(resp) if resp.status == 204 => {
                    let sub = parts[..=i].join(".");
                    return Ok((domain, sub));
                }
                Ok(_) => continue,
                Err(e) => return Err(Error::Provider(format!("nederhost zone resolution: {e}"))),
            }
        }
        Err(Error::Provider(format!("nederhost: no zone found for {fulldomain}")))
    }
}
