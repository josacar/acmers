use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Acmeproxy;

impl DnsProvider for Acmeproxy {
    fn slug() -> &'static str {
        "acmeproxy"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ACMEPROXY_URL_BASE", "ACMEPROXY_USERNAME", "ACMEPROXY_PASSWORD"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Acmeproxy))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
