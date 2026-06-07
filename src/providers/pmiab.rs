use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Pmiab;

impl DnsProvider for Pmiab {
    fn slug() -> &'static str {
        "pmiab"
    }

    fn env_vars() -> &'static [&'static str] {
        &["PMIAB_Username", "PMIAB_Password", "PMIAB_Server"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Pmiab))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
