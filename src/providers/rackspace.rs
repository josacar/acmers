use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Rackspace;

impl DnsProvider for Rackspace {
    fn slug() -> &'static str {
        "rackspace"
    }

    fn env_vars() -> &'static [&'static str] {
        &["RACKSPACE_Username", "RACKSPACE_ApiKey"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Rackspace))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
