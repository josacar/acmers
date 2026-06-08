use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Nsupdate;

impl DnsProvider for Nsupdate {
    fn slug() -> &'static str {
        "nsupdate"
    }

    fn env_vars() -> &'static [&'static str] {
        &["NSUPDATE_SERVER", "NSUPDATE_KEY"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Nsupdate))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider("RFC 2136 DNS UPDATE protocol requires direct socket access to port 53. Not yet implemented.".into()))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
