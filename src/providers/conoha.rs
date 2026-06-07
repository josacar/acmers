use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Conoha;

impl DnsProvider for Conoha {
    fn slug() -> &'static str {
        "conoha"
    }

    fn env_vars() -> &'static [&'static str] {
        &["CONOHA_Username", "CONOHA_Password", "CONOHA_TenantId", "CONOHA_IdentityServiceApi"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Conoha))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
