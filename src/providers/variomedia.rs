use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Variomedia;

impl DnsProvider for Variomedia {
    fn slug() -> &'static str {
        "variomedia"
    }

    fn env_vars() -> &'static [&'static str] {
        &["VARIOMEDIA_Email", "VARIOMEDIA_Token"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Variomedia))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
