use std::collections::HashMap;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Acmeproxy {
    url_base: String,
    username: String,
    password: String,
}

impl DnsProvider for Acmeproxy {
    fn slug() -> &'static str {
        "acmeproxy"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ACMEPROXY_URL_BASE", "ACMEPROXY_USERNAME", "ACMEPROXY_PASSWORD"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let url_base = env.get("ACMEPROXY_URL_BASE")
            .ok_or_else(|| Error::Config("ACMEPROXY_URL_BASE required".into()))?
            .clone();
        let username = env.get("ACMEPROXY_USERNAME")
            .ok_or_else(|| Error::Config("ACMEPROXY_USERNAME required".into()))?
            .clone();
        let password = env.get("ACMEPROXY_PASSWORD")
            .ok_or_else(|| Error::Config("ACMEPROXY_PASSWORD required".into()))?
            .clone();
        Ok(Box::new(Acmeproxy { url_base, username, password }))
    }

    fn add_txt(&self, _domain: &str, _name: &str, value: &str) -> ProviderResult {
        let basic_auth = make_basic(&self.username, &self.password);
        let url = format!("{}/update", self.url_base.trim_end_matches('/'));
        let body = serde_json::json!({
            "subdomain": "",
            "txt": value,
        });
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &basic_auth)])
            .map_err(|e| Error::Provider(format!("acmeproxy update: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("acmeproxy update: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}

fn make_basic(username: &str, password: &str) -> String {
    let creds = format!("{username}:{password}");
    format!("Basic {}", base64::encode(creds.as_bytes()))
}
