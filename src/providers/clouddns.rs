use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Clouddns;

impl DnsProvider for Clouddns {
    fn slug() -> &'static str {
        "clouddns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["CLOUDDNS_CLIENT_ID", "CLOUDDNS_EMAIL", "CLOUDDNS_PASSWORD"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Clouddns))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
