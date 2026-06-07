use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Schlundtech;

impl DnsProvider for Schlundtech {
    fn slug() -> &'static str {
        "schlundtech"
    }

    fn env_vars() -> &'static [&'static str] {
        &["SCHLUNDTECH_Username", "SCHLUNDTECH_Password"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Schlundtech))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
