use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Googledomains;

impl DnsProvider for Googledomains {
    fn slug() -> &'static str {
        "googledomains"
    }

    fn env_vars() -> &'static [&'static str] {
        &["GOOGLEDOMAINS_ACCESS_TOKEN"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Googledomains))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
