use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Hestiacp;

impl DnsProvider for Hestiacp {
    fn slug() -> &'static str {
        "hestiacp"
    }

    fn env_vars() -> &'static [&'static str] {
        &["HESTIACP_USERNAME", "HESTIACP_PASSWORD", "HESTIACP_HOST"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Hestiacp))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider(format!("{} not yet implemented - please contribute at https://github.com/josacar/acmers", Self::slug())))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
