use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://spaceship.dev/api/v1";

pub struct Spaceship {
    api_key: String,
    api_secret: String,
}

impl DnsProvider for Spaceship {
    fn slug() -> &'static str {
        "spaceship"
    }

    fn env_vars() -> &'static [&'static str] {
        &["SPACESHIP_API_KEY", "SPACESHIP_API_SECRET", "SPACESHIP_ROOT_DOMAIN"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("SPACESHIP_API_KEY")
            .ok_or_else(|| Error::Config("SPACESHIP_API_KEY required".into()))?
            .clone();
        let api_secret = env.get("SPACESHIP_API_SECRET")
            .ok_or_else(|| Error::Config("SPACESHIP_API_SECRET required".into()))?
            .clone();
        Ok(Box::new(Spaceship { api_key, api_secret }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let root = self.get_root(domain)?;
        let subdomain = name.strip_suffix(&format!(".{root}")).unwrap_or(name);

        let body = serde_json::json!({
            "force": true,
            "items": [{
                "type": "TXT",
                "name": subdomain,
                "value": value,
                "ttl": 600,
            }]
        });
        let url = format!("{BASE_URL}/dns/records/{root}");
        let resp = http::put(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("X-API-Key", &self.api_key), ("X-API-Secret", &self.api_secret)])
            .map_err(|e| Error::Provider(format!("spaceship add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("spaceship add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let root = match self.get_root(domain) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let subdomain = name.strip_suffix(&format!(".{root}")).unwrap_or(name);

        let body = serde_json::json!([{
            "type": "TXT",
            "name": subdomain,
            "value": value,
        }]);
        let url = format!("{BASE_URL}/dns/records/{root}");
        match http::delete_with_body(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("X-API-Key", &self.api_key), ("X-API-Secret", &self.api_secret)]) {
            Ok(_) => Ok(()),
            Err(_) => Ok(()),
        }
    }
}

impl Spaceship {
    fn get_root(&self, domain: &str) -> Result<String, Error> {
        if let Some(root) = std::env::var_os("SPACESHIP_ROOT_DOMAIN") {
            if let Some(root) = root.to_str() {
                if !root.is_empty() {
                    return Ok(root.to_string());
                }
            }
        }

        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len() {
            let zone = parts[i..].join(".");
            if zone.is_empty() {
                break;
            }
            let url = format!("{BASE_URL}/dns/records/{zone}?take=1&skip=0");
            if let Ok(resp) = http::get(&url, &[("X-API-Key", &self.api_key), ("X-API-Secret", &self.api_secret)]) {
                if resp.status == 200 {
                    return Ok(zone);
                }
            }
        }

        Err(Error::Provider(format!("spaceship: could not detect root zone for '{domain}'. Set SPACESHIP_ROOT_DOMAIN manually.")))
    }
}
