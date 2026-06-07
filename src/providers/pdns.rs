use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Powerdns;

impl DnsProvider for Powerdns {
    fn slug() -> &'static str {
        "pdns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["PDNS_Url", "PDNS_ServerId", "PDNS_Token", "PDNS_Ttl"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Powerdns))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
