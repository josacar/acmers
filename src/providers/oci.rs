use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Oci;

impl DnsProvider for Oci {
    fn slug() -> &'static str {
        "oci"
    }

    fn env_vars() -> &'static [&'static str] {
        &["OCI_PRIVKEY", "OCI_TENANCY", "OCI_USER", "OCI_REGION", "OCI_COMPARTMENT"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Oci))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider(format!("{} not yet implemented - please contribute at https://github.com/josacar/acmers", Self::slug())))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
