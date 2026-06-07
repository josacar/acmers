use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Hosting1984;

impl DnsProvider for Hosting1984 {
    fn slug() -> &'static str {
        "hosting1984"
    }

    fn env_vars() -> &'static [&'static str] {
        &["FOURTH1984_USERNAME", "FOURTH1984_PASSWORD"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Hosting1984))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
