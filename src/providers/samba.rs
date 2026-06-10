use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Samba;

impl DnsProvider for Samba {
    fn slug() -> &'static str {
        "samba"
    }

    fn env_vars() -> &'static [&'static str] {
        &["SAMBA_HOSTNAME", "SAMBA_DOMAIN", "SAMBA_USERNAME", "SAMBA_PASSWORD"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Err(Error::Provider(
            "samba provider requires samba-tool CLI and cannot be implemented in pure Rust".into(),
        ))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider("samba provider requires samba-tool CLI".into()))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider("samba provider requires samba-tool CLI".into()))
    }
}
