use std::collections::HashMap;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Lexicon;

impl DnsProvider for Lexicon {
    fn slug() -> &'static str {
        "lexicon"
    }

    fn env_vars() -> &'static [&'static str] {
        &["LEXICON_Provider"]
    }

    fn new(_env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Lexicon))
    }

    fn add_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Err(Error::Provider("Lexicon Delegation requires the Lexicon CLI tool. Install lexicon via pip and use its CLI directly.".into()))
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}
