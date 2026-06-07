use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Pdnsmanager;

impl DnsProvider for Pdnsmanager {
    fn slug() -> &'static str {
        "pdnsmanager"
    }

    fn env_vars() -> &'static [&'static str] {
        &["PDNSMGR_API_KEY", "PDNSMGR_API_PASSWORD", "PDNSMGR_API_URL"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Pdnsmanager))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
