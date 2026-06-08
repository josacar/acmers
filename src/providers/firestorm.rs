use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Firestorm {
    api_user: String,
    api_key: String,
    base_url: String,
}

impl DnsProvider for Firestorm {
    fn slug() -> &'static str {
        "firestorm"
    }

    fn env_vars() -> &'static [&'static str] {
        &["FST_Key", "FST_Secret", "FST_Url"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_user = env.get("FST_Key")
            .ok_or_else(|| Error::Config("FST_Key required".into()))?
            .clone();
        let api_key = env.get("FST_Secret")
            .ok_or_else(|| Error::Config("FST_Secret required".into()))?
            .clone();
        let base_url = env.get("FST_Url")
            .cloned()
            .unwrap_or_else(|| "https://api.firestorm.ch/acme-dns".into());
        Ok(Box::new(Firestorm { api_user, api_key, base_url }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let subdomain = name.strip_prefix("_acme-challenge.").unwrap_or(name);
        let url = format!("{}/update", self.base_url);
        let headers: &[(&str, &str)] = &[
            ("X-Api-User", &self.api_user),
            ("X-Api-Key", &self.api_key),
        ];
        let body = serde_json::json!({
            "subdomain": subdomain,
            "txt": value,
        });
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("firestorm add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("firestorm add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        if !resp.body.contains(value) {
            return Err(Error::Provider(format!("firestorm add TXT: unexpected response: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let subdomain = name.strip_prefix("_acme-challenge.").unwrap_or(name);
        let url = format!("{}/remove", self.base_url);
        let headers: &[(&str, &str)] = &[
            ("X-Api-User", &self.api_user),
            ("X-Api-Key", &self.api_key),
        ];
        let body = serde_json::json!({
            "subdomain": subdomain,
            "txt": value,
        });
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("firestorm remove TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("firestorm remove TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }
}
