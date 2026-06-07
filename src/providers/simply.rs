use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Simply;

impl DnsProvider for Simply {
    fn slug() -> &'static str {
        "simply"
    }

    fn env_vars() -> &'static [&'static str] {
        &["SIMPLY_ApiLogin", "SIMPLY_ApiKey"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Simply))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
