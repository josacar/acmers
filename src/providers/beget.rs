use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Beget;

impl DnsProvider for Beget {
    fn slug() -> &'static str {
        "beget"
    }

    fn env_vars() -> &'static [&'static str] {
        &["BEGET_USERNAME", "BEGET_PASSWORD"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Beget))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider(format!("{} requires a login-then-token authentication flow which is not yet supported", Self::slug())))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
