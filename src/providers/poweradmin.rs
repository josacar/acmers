use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Poweradmin;

impl DnsProvider for Poweradmin {
    fn slug() -> &'static str {
        "poweradmin"
    }

    fn env_vars() -> &'static [&'static str] {
        &["POWERADMIN_Username", "POWERADMIN_Password", "POWERADMIN_Hostname"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Poweradmin))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider(format!("{} not yet implemented - please contribute at https://github.com/josacar/acmers", Self::slug())))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
