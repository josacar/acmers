use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Cpanel;

impl DnsProvider for Cpanel {
    fn slug() -> &'static str {
        "cpanel"
    }

    fn env_vars() -> &'static [&'static str] {
        &["CPANEL_Hostname", "CPANEL_Username", "CPANEL_ApiToken"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Cpanel))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
