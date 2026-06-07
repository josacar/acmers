use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Internetbs {
    api_key: String,
    api_password: String,
}

impl DnsProvider for Internetbs {
    fn slug() -> &'static str {
        "internetbs"
    }

    fn env_vars() -> &'static [&'static str] {
        &["INTERNETBS_API_KEY", "INTERNETBS_API_PASSWORD"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("INTERNETBS_API_KEY")
            .ok_or_else(|| Error::Config("INTERNETBS_API_KEY required".into()))?
            .clone();
        let api_password = env.get("INTERNETBS_API_PASSWORD")
            .ok_or_else(|| Error::Config("INTERNETBS_API_PASSWORD required".into()))?
            .clone();
        Ok(Box::new(Internetbs { api_key, api_password }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let base = "https://api.internet.bs";
        let url = format!("{base}/Domain/DnsRecord/Add?ApiKey={}&Password={}&FullDomainName={domain}&HostName={name}&Type=TXT&Value={value}&Ttl=120",
            self.api_key, self.api_password);
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("internetbs add: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("internetbs add: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let base = "https://api.internet.bs";
        let url = format!("{base}/Domain/DnsRecord/Remove?ApiKey={}&Password={}&FullDomainName={domain}&HostName={name}&Type=TXT&Value={value}",
            self.api_key, self.api_password);
        http::get(&url, &[]).ok();
        Ok(())
    }
}
