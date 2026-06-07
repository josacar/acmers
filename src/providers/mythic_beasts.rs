use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct MythicBeasts;

impl DnsProvider for MythicBeasts {
    fn slug() -> &'static str {
        "mythic_beasts"
    }

    fn env_vars() -> &'static [&'static str] {
        &["MYTHIC_BEASTS_Key", "MYTHIC_BEASTS_Secret"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(MythicBeasts))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
