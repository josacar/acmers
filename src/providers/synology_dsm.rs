use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct SynologyDsm;

impl DnsProvider for SynologyDsm {
    fn slug() -> &'static str {
        "synology_dsm"
    }

    fn env_vars() -> &'static [&'static str] {
        &["SYNOLOGY_DSM_HOSTNAME", "SYNOLOGY_DSM_USERNAME", "SYNOLOGY_DSM_PASSWORD"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(SynologyDsm))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider(format!("{} not yet implemented - please contribute at https://github.com/josacar/acmers", Self::slug())))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
