use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Infoblox;

impl DnsProvider for Infoblox {
    fn slug() -> &'static str {
        "infoblox"
    }

    fn env_vars() -> &'static [&'static str] {
        &["Infoblox_Creds", "Infoblox_Server"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Infoblox))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
