use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Dnsmadeeasy;

impl DnsProvider for Dnsmadeeasy {
    fn slug() -> &'static str {
        "me"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ME_Key", "ME_Secret"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Dnsmadeeasy))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
