use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Pleskxml;

impl DnsProvider for Pleskxml {
    fn slug() -> &'static str {
        "pleskxml"
    }

    fn env_vars() -> &'static [&'static str] {
        &["PLESKXML_Username", "PLESKXML_Password", "PLESKXML_Host"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Pleskxml))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider(format!("{} not yet implemented - please contribute at https://github.com/josacar/acmers", Self::slug())))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
