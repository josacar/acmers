use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Yandex360;

impl DnsProvider for Yandex360 {
    fn slug() -> &'static str {
        "yandex360"
    }

    fn env_vars() -> &'static [&'static str] {
        &["YANDEX360_CLIENT_ID", "YANDEX360_CLIENT_SECRET"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Yandex360))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
