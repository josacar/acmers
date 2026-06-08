use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct InfobloxUddi;

impl DnsProvider for InfobloxUddi {
    fn slug() -> &'static str {
        "infoblox_uddi"
    }

    fn env_vars() -> &'static [&'static str] {
        &["INFOBLOX_UDDI_CREDENTIALS", "INFOBLOX_UDDI_SERVER"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(InfobloxUddi))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider("Infoblox UDDI requires custom integration. Not yet implemented.".into()))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
