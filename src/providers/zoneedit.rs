use std::collections::HashMap;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Zoneedit {
    basic_auth: String,
}

impl DnsProvider for Zoneedit {
    fn slug() -> &'static str {
        "zoneedit"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ZONEEDIT_ID", "ZONEEDIT_Token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let id = env.get("ZONEEDIT_ID")
            .ok_or_else(|| Error::Config("ZONEEDIT_ID required".into()))?
            .clone();
        let token = env.get("ZONEEDIT_Token")
            .ok_or_else(|| Error::Config("ZONEEDIT_Token required".into()))?
            .clone();
        let creds = format!("{id}:{token}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Zoneedit { basic_auth }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let url = format!(
            "https://dynamic.zoneedit.com/txt-create.php?host={}&rdata={}",
            name, value
        );
        Self::api_call(&self.basic_auth, &url, false)
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let url = format!(
            "https://dynamic.zoneedit.com/txt-delete.php?host={}&rdata={}",
            name, value
        );
        Self::api_call(&self.basic_auth, &url, true)
    }
}

impl Zoneedit {
    fn api_call(basic_auth: &str, url: &str, is_delete: bool) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", basic_auth)];
        let mut tries = 3u8;
        loop {
            tries -= 1;
            let resp = http::get(url, headers)
                .map_err(|e| Error::Provider(format!("zoneedit: {e}")))?;
            if resp.body.contains("SUCCESS") && resp.body.contains("200") {
                if is_delete {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
                return Ok(());
            }
            if tries == 0 {
                return Err(Error::Provider(format!(
                    "zoneedit: exhausted retries, last response: {}", resp.body
                )));
            }
            let wait = Self::parse_ratelimit(&resp.body).unwrap_or(10);
            std::thread::sleep(std::time::Duration::from_secs(wait));
        }
    }

    fn parse_ratelimit(body: &str) -> Option<u64> {
        let marker = "Minimum ";
        let start = body.find(marker)? + marker.len();
        let rest = &body[start..];
        let end = rest.find(' ')?;
        rest[..end].parse::<u64>().ok()
    }
}
