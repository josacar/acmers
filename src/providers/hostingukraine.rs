use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Hostingukraine;

impl DnsProvider for Hostingukraine {
    fn slug() -> &'static str {
        "hostingukraine"
    }

    fn env_vars() -> &'static [&'static str] {
        &["HOSTINGUKRAINE_UUID", "HOSTINGUKRAINE_TOKEN"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Hostingukraine))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
