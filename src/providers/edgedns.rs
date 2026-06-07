use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Edgedns;

impl DnsProvider for Edgedns {
    fn slug() -> &'static str {
        "edgedns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["AKAMAI_ACCESS_TOKEN", "AKAMAI_CLIENT_TOKEN", "AKAMAI_CLIENT_SECRET", "AKAMAI_HOST", "AKAMAI_EDGERC_CONTENT"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Edgedns))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
