use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.ip-projects.de/v1/dns/acme";

pub struct Ipprojects {
    api_key: String,
}

impl DnsProvider for Ipprojects {
    fn slug() -> &'static str { "ipprojects" }
    fn env_vars() -> &'static [&'static str] { &["IPP_Apikey"] }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Ipprojects {
            api_key: env.get("IPP_Apikey")
                .ok_or_else(|| Error::Config("IPP_Apikey required".into()))?
                .clone(),
        }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let body = serde_json::json!({
            "domain": name,
            "key": name,
            "value": value,
        });
        let url = format!("{BASE_URL}/add");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("X-API-Key", &self.api_key)])
            .map_err(|e| Error::Provider(format!("ipprojects add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("ipprojects add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let body = serde_json::json!({
            "domain": name,
            "key": name,
            "value": value,
        });
        let url = format!("{BASE_URL}/remove");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("X-API-Key", &self.api_key)])
            .map_err(|e| Error::Provider(format!("ipprojects remove TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("ipprojects remove TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }
}
