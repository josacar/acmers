use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Geoscaling;

impl DnsProvider for Geoscaling {
    fn slug() -> &'static str {
        "geoscaling"
    }

    fn env_vars() -> &'static [&'static str] {
        &["GEOSCALING_Username", "GEOSCALING_Password"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Geoscaling))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
