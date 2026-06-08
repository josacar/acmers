use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Linode;

impl DnsProvider for Linode {
    fn slug() -> &'static str {
        "linode"
    }

    fn env_vars() -> &'static [&'static str] {
        &["LINODE_API_KEY"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Linode))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider("Linode API v3 is deprecated. Use `--dns linode_v4` with LINODE_V4_API_KEY instead.".into()))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
