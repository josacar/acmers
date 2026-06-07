use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Myapi {
    token: String,
    endpoint: String,
}

impl DnsProvider for Myapi {
    fn slug() -> &'static str {
        "myapi"
    }

    fn env_vars() -> &'static [&'static str] {
        &["MYAPI_Token", "MYAPI_Endpoint"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("MYAPI_Token")
            .ok_or_else(|| Error::Config("MYAPI_Token required".into()))?
            .clone();
        let endpoint = env.get("MYAPI_Endpoint")
            .ok_or_else(|| Error::Config("MYAPI_Endpoint required".into()))?
            .trim_end_matches('/')
            .to_string();
        Ok(Box::new(Myapi { token, endpoint }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let body = serde_json::json!({
            "domain": domain,
            "name": name,
            "value": value,
        });
        let url = format!("{}/add_txt", self.endpoint);
        let auth = format!("Bearer {}", self.token);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[
            ("Authorization", &auth),
        ]).map_err(|e| Error::Provider(format!("myapi add_txt: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("myapi add_txt: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let body = serde_json::json!({
            "domain": domain,
            "name": name,
            "value": value,
        });
        let url = format!("{}/remove_txt", self.endpoint);
        let auth = format!("Bearer {}", self.token);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[
            ("Authorization", &auth),
        ]).map_err(|e| Error::Provider(format!("myapi remove_txt: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("myapi remove_txt: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }
}
