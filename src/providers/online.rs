use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Online;

impl DnsProvider for Online {
    fn slug() -> &'static str {
        "online"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ONLINE_API_KEY"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Online))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider("Online.net is now Scaleway. Use `--dns scaleway` instead.".into()))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
