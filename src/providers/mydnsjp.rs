use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Mydnsjp;

impl DnsProvider for Mydnsjp {
    fn slug() -> &'static str {
        "mydnsjp"
    }

    fn env_vars() -> &'static [&'static str] {
        &["MYDNSJP_MasterID", "MYDNSJP_MasterPassword"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Mydnsjp))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
