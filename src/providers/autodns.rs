use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Autodns;

impl DnsProvider for Autodns {
    fn slug() -> &'static str {
        "autodns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["AUTODNS_USER", "AUTODNS_PASSWORD", "AUTODNS_CONTEXT"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Autodns))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider("InternetX autoDNS uses XML API. Not yet implemented.".into()))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
