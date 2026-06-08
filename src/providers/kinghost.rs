use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.kinghost.net/acme";

pub struct Kinghost {
    username: String,
    password: String,
}

impl DnsProvider for Kinghost {
    fn slug() -> &'static str {
        "kinghost"
    }

    fn env_vars() -> &'static [&'static str] {
        &["KINGHOST_Username", "KINGHOST_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("KINGHOST_Username")
            .ok_or_else(|| Error::Config("KINGHOST_Username required".into()))?
            .clone();
        let password = env.get("KINGHOST_Password")
            .ok_or_else(|| Error::Config("KINGHOST_Password required".into()))?
            .clone();
        Ok(Box::new(Kinghost { username, password }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let form = format!("name={name}&content={value}");
        let url = format!("{BASE_URL}/dns.json");
        let headers: &[(&str, &str)] = &[
            ("X-Auth-Email", &self.username),
            ("X-Auth-Key", &self.password),
        ];

        let check_url = format!("{url}?{form}");
        let resp = http::get(&check_url, headers)
            .map_err(|e| Error::Provider(format!("kinghost check TXT: {e}")))?;
        if !resp.body.contains("\"status\":\"ok\"") {
            return Err(Error::Provider(format!("kinghost check TXT: {}", resp.body)));
        }

        let resp = http::post(&url, form.as_bytes(), "application/x-www-form-urlencoded", headers)
            .map_err(|e| Error::Provider(format!("kinghost add TXT: {e}")))?;
        if !resp.body.contains("\"status\":\"ok\"") {
            return Err(Error::Provider(format!("kinghost add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let form = format!("name={name}&content={value}");
        let url = format!("{BASE_URL}/dns.json");
        let headers: &[(&str, &str)] = &[
            ("X-Auth-Email", &self.username),
            ("X-Auth-Key", &self.password),
        ];

        let resp = http::delete_with_body(&url, form.as_bytes(), "application/x-www-form-urlencoded", headers)
            .map_err(|e| Error::Provider(format!("kinghost remove TXT: {e}")))?;
        if !resp.body.contains("\"status\":\"ok\"") {
            return Err(Error::Provider(format!("kinghost remove TXT: {}", resp.body)));
        }
        Ok(())
    }
}
