use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Technitium;

impl DnsProvider for Technitium {
    fn slug() -> &'static str {
        "technitium"
    }

    fn env_vars() -> &'static [&'static str] {
        &["TECHNITIUM_Server", "TECHNITIUM_Token"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Technitium))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
