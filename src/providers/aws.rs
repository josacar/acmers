use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Route53;

impl DnsProvider for Route53 {
    fn slug() -> &'static str {
        "aws"
    }

    fn env_vars() -> &'static [&'static str] {
        &["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Route53))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
