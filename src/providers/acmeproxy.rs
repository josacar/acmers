use std::collections::HashMap;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Acmeproxy {
    endpoint: String,
    username: Option<String>,
    password: Option<String>,
}

impl DnsProvider for Acmeproxy {
    fn slug() -> &'static str {
        "acmeproxy"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ACMEPROXY_ENDPOINT", "ACMEPROXY_USERNAME", "ACMEPROXY_PASSWORD"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let endpoint = env.get("ACMEPROXY_ENDPOINT")
            .ok_or_else(|| Error::Config("ACMEPROXY_ENDPOINT required".into()))?
            .clone();
        let username = env.get("ACMEPROXY_USERNAME").cloned();
        let password = env.get("ACMEPROXY_PASSWORD").cloned();
        Ok(Box::new(Acmeproxy { endpoint, username, password }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let fqdn = format!("{}.{}.", name, domain);
        let url = format!("{}/present", self.endpoint.trim_end_matches('/'));
        let body = serde_json::json!({
            "fqdn": fqdn,
            "value": value,
        });
        let headers = self.auth_headers();
        let header_refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &header_refs)
            .map_err(|e| Error::Provider(format!("acmeproxy present: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("acmeproxy present: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let fqdn = format!("{}.{}.", name, domain);
        let url = format!("{}/cleanup", self.endpoint.trim_end_matches('/'));
        let body = serde_json::json!({
            "fqdn": fqdn,
            "value": value,
        });
        let headers = self.auth_headers();
        let header_refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &header_refs)
            .map_err(|e| Error::Provider(format!("acmeproxy cleanup: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("acmeproxy cleanup: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }
}

impl Acmeproxy {
    fn auth_headers(&self) -> Vec<(String, String)> {
        let mut headers = vec![
            ("Accept".to_string(), "application/json".to_string()),
            ("Content-Type".to_string(), "application/json".to_string()),
        ];
        if let (Some(user), Some(pass)) = (&self.username, &self.password) {
            if !user.is_empty() && !pass.is_empty() {
                let creds = format!("{user}:{pass}");
                let encoded = base64::encode_std(creds.as_bytes());
                headers.insert(0, ("Authorization".to_string(), format!("Basic {encoded}")));
            }
        }
        headers
    }
}
