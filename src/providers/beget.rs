use std::collections::HashMap;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.beget.com";

pub struct Beget {
    auth: String,
}

impl DnsProvider for Beget {
    fn slug() -> &'static str {
        "beget"
    }

    fn env_vars() -> &'static [&'static str] {
        &["BEGET_USERNAME", "BEGET_PASSWORD"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("BEGET_USERNAME")
            .ok_or_else(|| Error::Config("BEGET_USERNAME required".into()))?
            .clone();
        let password = env.get("BEGET_PASSWORD")
            .ok_or_else(|| Error::Config("BEGET_PASSWORD required".into()))?
            .clone();
        let creds = format!("{username}:{password}");
        let auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Beget { auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let url = format!("{BASE_URL}/v1/dns/{}", domain);
        let headers: &[(&str, &str)] = &[("Authorization", &self.auth)];
        let body = serde_json::json!({
            "type": "TXT",
            "name": name,
            "content": value,
            "ttl": 120,
        });
        match http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers) {
            Ok(resp) if resp.status < 400 => Ok(()),
            _ => Err(Error::Provider("Beget.com requires session-based API access. Not yet implemented.".into())),
        }
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
