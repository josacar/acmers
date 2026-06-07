use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Opnsense;

impl DnsProvider for Opnsense {
    fn slug() -> &'static str {
        "opnsense"
    }

    fn env_vars() -> &'static [&'static str] {
        &["OPNSENSE_API_KEY", "OPNSENSE_API_SECRET", "OPNSENSE_HOST"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Opnsense))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider(format!("{} not yet implemented - please contribute at https://github.com/josacar/acmers", Self::slug())))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
