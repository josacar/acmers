use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Dnsexit;

impl DnsProvider for Dnsexit {
    fn slug() -> &'static str {
        "dnsexit"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DNSEXIT_API_KEY", "DNSEXIT_API_Key"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Dnsexit))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
