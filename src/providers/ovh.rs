use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Ovh;

impl DnsProvider for Ovh {
    fn slug() -> &'static str {
        "ovh"
    }

    fn env_vars() -> &'static [&'static str] {
        &["OVH_AK", "OVH_AS", "OVH_CK", "OVH_END_POINT"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Ovh))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
