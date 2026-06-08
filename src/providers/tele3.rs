use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://www.tele3.cz/acme/";

pub struct Tele3 {
    key: String,
    secret: String,
}

impl DnsProvider for Tele3 {
    fn slug() -> &'static str { "tele3" }
    fn env_vars() -> &'static [&'static str] { &["TELE3_Key", "TELE3_Secret"] }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let key = env.get("TELE3_Key")
            .ok_or_else(|| Error::Config("TELE3_Key required".into()))?.clone();
        let secret = env.get("TELE3_Secret")
            .ok_or_else(|| Error::Config("TELE3_Secret required".into()))?.clone();
        Ok(Box::new(Tele3 { key, secret }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let body = serde_json::json!({
            "key": self.key,
            "secret": self.secret,
            "ope": "add",
            "domain": name,
            "value": value,
        });
        let resp = http::post(BASE_URL, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("tele3 add TXT: {e}")))?;
        if resp.body.trim() != "success" {
            return Err(Error::Provider(format!("tele3 add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let body = serde_json::json!({
            "key": self.key,
            "secret": self.secret,
            "ope": "rm",
            "domain": name,
            "value": value,
        });
        let resp = http::post(BASE_URL, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("tele3 remove TXT: {e}")))?;
        if resp.body.trim() != "success" {
            eprintln!("warning: tele3 remove TXT: {}", resp.body);
        }
        Ok(())
    }
}
