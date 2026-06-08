use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Bookmyname {
    username: String,
    password: String,
}

impl DnsProvider for Bookmyname {
    fn slug() -> &'static str {
        "bookmyname"
    }

    fn env_vars() -> &'static [&'static str] {
        &["BOOKMYNAME_Username", "BOOKMYNAME_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("BOOKMYNAME_Username")
            .ok_or_else(|| Error::Config("BOOKMYNAME_Username required".into()))?.clone();
        let password = env.get("BOOKMYNAME_Password")
            .ok_or_else(|| Error::Config("BOOKMYNAME_Password required".into()))?.clone();
        Ok(Box::new(Bookmyname { username, password }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let url = format!(
            "https://{}:{}@www.bookmyname.com/dyndns/?hostname={}&type=TXT&ttl=300&do=add&value={}",
            self.username, self.password, name, value
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("bookmyname add TXT: {e}")))?;
        if !resp.body.starts_with("good: update done, cid ") {
            return Err(Error::Provider(format!("bookmyname add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let url = format!(
            "https://{}:{}@www.bookmyname.com/dyndns/?hostname={}&type=TXT&ttl=300&do=remove&value={}",
            self.username, self.password, name, value
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("bookmyname remove TXT: {e}")))?;
        if !resp.body.starts_with("good: remove done 1, cid ") {
            eprintln!("warning: bookmyname remove TXT: {}", resp.body);
        }
        Ok(())
    }
}
