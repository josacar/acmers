use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Anikeen;

impl DnsProvider for Anikeen {
    fn slug() -> &'static str {
        "anikeen"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ANIKEEN_USERNAME", "ANIKEEN_PASSWORD"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Anikeen))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
