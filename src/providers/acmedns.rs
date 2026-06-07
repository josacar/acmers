use std::collections::HashMap;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Acmedns {
    url_base: String,
    username: String,
    password: String,
    subdomain: String,
}

impl DnsProvider for Acmedns {
    fn slug() -> &'static str {
        "acmedns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ACMEDNS_URL_BASE", "ACMEDNS_USERNAME", "ACMEDNS_PASSWORD", "ACMEDNS_SUBDOMAIN"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let url_base = env.get("ACMEDNS_URL_BASE")
            .ok_or_else(|| Error::Config("ACMEDNS_URL_BASE required".into()))?
            .clone();
        let username = env.get("ACMEDNS_USERNAME")
            .ok_or_else(|| Error::Config("ACMEDNS_USERNAME required".into()))?
            .clone();
        let password = env.get("ACMEDNS_PASSWORD")
            .ok_or_else(|| Error::Config("ACMEDNS_PASSWORD required".into()))?
            .clone();
        let subdomain = env.get("ACMEDNS_SUBDOMAIN")
            .ok_or_else(|| Error::Config("ACMEDNS_SUBDOMAIN required".into()))?
            .clone();
        Ok(Box::new(Acmedns { url_base, username, password, subdomain }))
    }

    fn add_txt(&self, _domain: &str, _name: &str, value: &str) -> ProviderResult {
        let basic_auth = make_basic(&self.username, &self.password);
        let url = format!("{}/update", self.url_base.trim_end_matches('/'));
        let body = serde_json::json!({
            "subdomain": self.subdomain,
            "txt": value,
        });
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &basic_auth)])
            .map_err(|e| Error::Provider(format!("acmedns update: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("acmedns update: {} {}", resp.status, resp.body)));
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
