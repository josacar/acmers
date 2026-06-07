use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct HeDdns;

impl DnsProvider for HeDdns {
    fn slug() -> &'static str {
        "he_ddns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["HE_DDNS_Key", "HE_DDNS_Secret"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(HeDdns))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
