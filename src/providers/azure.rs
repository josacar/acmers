use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Azure;

impl DnsProvider for Azure {
    fn slug() -> &'static str {
        "azure"
    }

    fn env_vars() -> &'static [&'static str] {
        &["AZUREDNS_SUBSCRIPTIONID", "AZUREDNS_TENANTID", "AZUREDNS_APPID", "AZUREDNS_CLIENTSECRET"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Azure))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
