use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Wedos;

impl DnsProvider for Wedos {
    fn slug() -> &'static str {
        "wedos"
    }

    fn env_vars() -> &'static [&'static str] {
        &["WEDOS_User", "WEDOS_Password"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Wedos))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider(format!("{} not yet implemented - please contribute at https://github.com/josacar/acmers", Self::slug())))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
