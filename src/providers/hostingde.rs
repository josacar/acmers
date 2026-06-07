use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Hostingde;

impl DnsProvider for Hostingde {
    fn slug() -> &'static str {
        "hostingde"
    }

    fn env_vars() -> &'static [&'static str] {
        &["HOSTINGDE_APIKEY", "HOSTINGDE_ENDPOINT"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Hostingde))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
