use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Openprovider;

impl DnsProvider for Openprovider {
    fn slug() -> &'static str {
        "openprovider"
    }

    fn env_vars() -> &'static [&'static str] {
        &["OPENPROVIDER_USER", "OPENPROVIDER_PASSWORD_HASH"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Openprovider))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
