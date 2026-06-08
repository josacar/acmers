use std::collections::HashMap;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.mgw-media.de/record";

pub struct Mgwm {
    customer: String,
    api_hash: String,
}

impl DnsProvider for Mgwm {
    fn slug() -> &'static str {
        "mgwm"
    }

    fn env_vars() -> &'static [&'static str] {
        &["MGWM_CUSTOMER", "MGWM_API_HASH"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let customer = env.get("MGWM_CUSTOMER")
            .ok_or_else(|| Error::Config("MGWM_CUSTOMER required".into()))?
            .clone();
        let api_hash = env.get("MGWM_API_HASH")
            .ok_or_else(|| Error::Config("MGWM_API_HASH required".into()))?
            .clone();
        Ok(Box::new(Mgwm { customer, api_hash }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = auth_header(&self.customer, &self.api_hash);
        let url = format!("{BASE_URL}/add/{name}/txt/{value}");
        let resp = http::get(&url, &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("mgwm add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("mgwm add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        if resp.body.trim() != "OK" {
            return Err(Error::Provider(format!("mgwm add TXT: unexpected response: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = auth_header(&self.customer, &self.api_hash);
        let url = format!("{BASE_URL}/rm/{name}/txt/{value}");
        let resp = match http::get(&url, &[("Authorization", &auth)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if resp.status >= 400 {
            return Ok(());
        }
        Ok(())
    }
}

fn auth_header(customer: &str, api_hash: &str) -> String {
    let creds = base64::encode_std(format!("{customer}:{api_hash}").as_bytes());
    format!("Basic {creds}")
}
