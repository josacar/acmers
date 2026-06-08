use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Unoeuro;

impl DnsProvider for Unoeuro {
    fn slug() -> &'static str {
        "unoeuro"
    }

    fn env_vars() -> &'static [&'static str] {
        &["UNOEURO_User", "UNOEURO_Password"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Unoeuro))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider("UnoEuro is now Simply.com. Use `--dns simply` instead with SIMPLY_ApiLogin and SIMPLY_ApiKey env vars.".into()))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
