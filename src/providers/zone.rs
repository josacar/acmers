use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Zone;

impl DnsProvider for Zone {
    fn slug() -> &'static str {
        "zone"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ZONE_Username", "ZONE_Key"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Zone))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider(format!("{} not yet implemented - please contribute at https://github.com/josacar/acmers", Self::slug())))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
