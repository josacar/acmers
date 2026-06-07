use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Gcloud;

impl DnsProvider for Gcloud {
    fn slug() -> &'static str {
        "gcloud"
    }

    fn env_vars() -> &'static [&'static str] {
        &["GCLOUD_PROJECT", "GCLOUD_SERVICE_ACCOUNT", "GCLOUD_ACCOUNT_TYPE"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Gcloud))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
