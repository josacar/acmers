use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Ispconfig;

impl DnsProvider for Ispconfig {
    fn slug() -> &'static str {
        "ispconfig"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ISPC_User", "ISPC_Password", "ISPC_Api", "ISPC_Api_Insecure"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Ispconfig))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
