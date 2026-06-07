use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct HeDdns {
    key: String,
    secret: String,
}

impl DnsProvider for HeDdns {
    fn slug() -> &'static str {
        "he_ddns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["HE_DDNS_Key", "HE_DDNS_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let key = env.get("HE_DDNS_Key")
            .ok_or_else(|| Error::Config("HE_DDNS_Key required".into()))?
            .clone();
        let secret = env.get("HE_DDNS_Secret")
            .ok_or_else(|| Error::Config("HE_DDNS_Secret required".into()))?
            .clone();
        Ok(Box::new(HeDdns { key, secret }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let hostname = format!("{name}.{domain}");
        let url = format!(
            "https://dyn.dns.he.net/nic/update?hostname={}&password={}&txt={}",
            urlencode(&hostname),
            urlencode(&self.key),
            urlencode(value),
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("HE DDNS add TXT: {e}")))?;
        if !resp.body.starts_with("good") && !resp.body.starts_with("nochg") {
            return Err(Error::Provider(format!("HE DDNS add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let hostname = format!("{name}.{domain}");
        let url = format!(
            "https://dyn.dns.he.net/nic/update?hostname={}&password={}&txt=",
            urlencode(&hostname),
            urlencode(&self.key),
        );
        http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("HE DDNS remove TXT: {e}")))?;
        Ok(())
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
