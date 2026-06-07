use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Efficientip;

impl DnsProvider for Efficientip {
    fn slug() -> &'static str {
        "efficientip"
    }

    fn env_vars() -> &'static [&'static str] {
        &["EFFICIENTIP_USERNAME", "EFFICIENTIP_PASSWORD", "EFFICIENTIP_HOSTNAME"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Efficientip))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
