use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Duckdns {
    token: String,
}

impl DnsProvider for Duckdns {
    fn slug() -> &'static str {
        "duckdns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DuckDNS_Token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("DuckDNS_Token")
            .ok_or_else(|| Error::Config("DuckDNS_Token required".into()))?
            .clone();
        Ok(Box::new(Duckdns { token }))
    }

    fn add_txt(&self, domain: &str, _name: &str, value: &str) -> ProviderResult {
        let url = format!(
            "https://www.duckdns.org/update?domains={domain}&token={}&txt={value}",
            self.token,
        );
        http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("DuckDNS update: {e}")))?;
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
