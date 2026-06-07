use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Tencent;

impl DnsProvider for Tencent {
    fn slug() -> &'static str {
        "tencent"
    }

    fn env_vars() -> &'static [&'static str] {
        &["TENCENT_SecretId", "TENCENT_SecretKey"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Tencent))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
