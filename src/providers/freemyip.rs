use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Freemyip {
    token: String,
}

impl DnsProvider for Freemyip {
    fn slug() -> &'static str {
        "freemyip"
    }

    fn env_vars() -> &'static [&'static str] {
        &["FREEMYIP_Token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("FREEMYIP_Token")
            .ok_or_else(|| Error::Config("FREEMYIP_Token required".into()))?
            .clone();
        Ok(Box::new(Freemyip { token }))
    }

    fn add_txt(&self, domain: &str, _name: &str, value: &str) -> ProviderResult {
        let url = format!("https://freemyip.com/update?token={}&domain={domain}&txt={value}", self.token);
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("freemyip update: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("freemyip update: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
