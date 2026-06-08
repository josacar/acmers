use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct One;

impl DnsProvider for One {
    fn slug() -> &'static str {
        "one"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ONE_Username", "ONE_Password"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(One))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider("One.com DNS API requires interactive login. Please contribute a provider at https://github.com/josacar/acmers".into()))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
