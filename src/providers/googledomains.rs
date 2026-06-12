use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Googledomains {
    access_token: String,
}

impl DnsProvider for Googledomains {
    fn slug() -> &'static str {
        "googledomains"
    }

    fn env_vars() -> &'static [&'static str] {
        &["GOOGLEDOMAINS_ACCESS_TOKEN"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let access_token = env.get("GOOGLEDOMAINS_ACCESS_TOKEN")
            .ok_or_else(|| Error::Config("GOOGLEDOMAINS_ACCESS_TOKEN required".into()))?
            .clone();
        Ok(Box::new(Googledomains { access_token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let root = self.find_root(domain)?;
        let url = format!("https://acmedns.googleapis.com/v1/acmeChallenges/{root}:start");
        let auth = format!("Bearer {}", self.access_token);
        let body = serde_json::to_vec(&serde_json::json!({
            "recordName": name,
            "digest": value,
        })).unwrap();
        let resp = http::post(&url, &body, "application/json",
            &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("Google Domains add: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Google Domains add: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let root = match self.find_root(domain) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let url = format!("https://acmedns.googleapis.com/v1/acmeChallenges/{root}:clear");
        let auth = format!("Bearer {}", self.access_token);
        let body = serde_json::to_vec(&serde_json::json!({
            "recordName": name,
            "digest": value,
        })).unwrap();
        let _ = http::post(&url, &body, "application/json",
            &[("Authorization", &auth)]);
        Ok(())
    }
}

impl Googledomains {
    fn find_root(&self, domain: &str) -> Result<String, Error> {
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 1..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                continue;
            }
            let auth = format!("Bearer {}", self.access_token);
            let url = format!("https://acmedns.googleapis.com/v1/acmeChallenges/{h}:start");
            let body = serde_json::to_vec(&serde_json::json!({
                "recordName": "_test",
                "digest": "test",
            })).unwrap();
            let resp = match http::post(&url, &body, "application/json",
                &[("Authorization", &auth)]) {
                Ok(r) => r,
                Err(_) => continue,
            };
            if resp.status < 400 {
                let clear_url = format!("https://acmedns.googleapis.com/v1/acmeChallenges/{h}:clear");
                let clear_body = serde_json::to_vec(&serde_json::json!({
                    "recordName": "_test",
                    "digest": "test",
                })).unwrap();
                let _ = http::post(&clear_url, &clear_body, "application/json",
                    &[("Authorization", &auth)]);
                return Ok(h);
            }
        }
        Err(Error::Provider(format!("Google Domains: zone not found for {domain}")))
    }
}
