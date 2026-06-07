use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Namecheap;

impl DnsProvider for Namecheap {
    fn slug() -> &'static str {
        "namecheap"
    }

    fn env_vars() -> &'static [&'static str] {
        &["NAMECHEAP_API_KEY", "NAMECHEAP_USERNAME"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Namecheap))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
