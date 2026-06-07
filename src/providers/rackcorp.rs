use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Rackcorp;

impl DnsProvider for Rackcorp {
    fn slug() -> &'static str {
        "rackcorp"
    }

    fn env_vars() -> &'static [&'static str] {
        &["RACKCORP_UUID", "RACKCORP_API_KEY"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Rackcorp))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
