use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Nsd;

impl DnsProvider for Nsd {
    fn slug() -> &'static str {
        "nsd"
    }

    fn env_vars() -> &'static [&'static str] {
        &["NSD_SERVER", "NSD_KEY"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Nsd))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider("NLnetLabs NSD uses DNS UPDATE protocol. Use nsupdate.".into()))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
