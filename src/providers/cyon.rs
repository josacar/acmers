use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Cyon;

impl DnsProvider for Cyon {
    fn slug() -> &'static str {
        "cyon"
    }

    fn env_vars() -> &'static [&'static str] {
        &["CY_Username", "CY_Password", "CY_OTP_Secret"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Cyon))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
