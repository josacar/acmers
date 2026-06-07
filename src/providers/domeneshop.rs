use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Domeneshop;

impl DnsProvider for Domeneshop {
    fn slug() -> &'static str {
        "domeneshop"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DOMENESHOP_Key", "DOMENESHOP_Secret"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Domeneshop))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
