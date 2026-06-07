use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Zoneedit;

impl DnsProvider for Zoneedit {
    fn slug() -> &'static str {
        "zoneedit"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ZONEEDIT_User", "ZONEEDIT_Token"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Zoneedit))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
