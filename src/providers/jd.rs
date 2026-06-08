use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Jd;

impl DnsProvider for Jd {
    fn slug() -> &'static str {
        "jd"
    }

    fn env_vars() -> &'static [&'static str] {
        &["JD_ACCESS_KEY", "JD_SECRET_KEY", "JD_REGION"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Jd))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider("JD Cloud uses complex signing. Not yet implemented.".into()))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
