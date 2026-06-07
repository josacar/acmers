use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct OpenproviderRest;

impl DnsProvider for OpenproviderRest {
    fn slug() -> &'static str {
        "openprovider_rest"
    }

    fn env_vars() -> &'static [&'static str] {
        &["OPENPROVIDER_REST_Username", "OPENPROVIDER_REST_Password"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(OpenproviderRest))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
