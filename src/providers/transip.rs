use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Transip;

impl DnsProvider for Transip {
    fn slug() -> &'static str {
        "transip"
    }

    fn env_vars() -> &'static [&'static str] {
        &["TRANSIP_Username", "TRANSIP_Key"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Transip))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider("TransIP uses RSA key signing for API access. Not yet implemented.".into()))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
