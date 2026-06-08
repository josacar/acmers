use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Hetzner;

impl DnsProvider for Hetzner {
    fn slug() -> &'static str {
        "hetzner"
    }

    fn env_vars() -> &'static [&'static str] {
        &["HETZNER_API_KEY"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Hetzner))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider("Hetzner Robot does not manage DNS zones. Use `--dns hetznercloud` for Hetzner Cloud DNS instead.".into()))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
