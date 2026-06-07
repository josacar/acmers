use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Nsupdate;

impl DnsProvider for Nsupdate {
    fn slug() -> &'static str {
        "nsupdate"
    }

    fn env_vars() -> &'static [&'static str] {
        &["NSUPDATE_SERVER", "NSUPDATE_KEY"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Nsupdate))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
