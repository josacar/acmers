use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Maradns;

impl DnsProvider for Maradns {
    fn slug() -> &'static str {
        "maradns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["MARADNS_URL"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Maradns))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider("MaraDNS uses CSV2 file-based zone management. Not yet implemented.".into()))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
