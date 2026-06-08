use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.nanelo.com/v1/";

pub struct Nanelo {
    token: String,
}

impl Nanelo {
    fn resolve_zone(&self, fulldomain: &str) -> Result<(String, String), Error> {
        let url = format!("{BASE_URL}{}/dns/getzones", self.token);
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("nanelo getzones: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("nanelo getzones: HTTP {} {}", resp.status, resp.body)));
        }
        let v: serde_json::Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Provider(format!("nanelo getzones: {e}")))?;
        let zones = v.get("zones")
            .and_then(|z| z.as_array())
            .ok_or_else(|| Error::Provider("nanelo getzones: missing zones array".into()))?;
        let mut best_zone = "";
        for z in zones {
            if let Some(zone) = z.as_str() {
                if fulldomain == zone || fulldomain.ends_with(&format!(".{zone}")) {
                    if zone.len() > best_zone.len() {
                        best_zone = zone;
                    }
                }
            }
        }
        if best_zone.is_empty() {
            return Err(Error::Provider(format!("nanelo: no matching zone for {fulldomain}")));
        }
        let sub_domain = if fulldomain == best_zone {
            String::new()
        } else {
            fulldomain[..fulldomain.len() - best_zone.len() - 1].to_string()
        };
        Ok((best_zone.to_string(), sub_domain))
    }
}

impl DnsProvider for Nanelo {
    fn slug() -> &'static str { "nanelo" }
    fn env_vars() -> &'static [&'static str] { &["NANELO_TOKEN"] }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("NANELO_TOKEN")
            .ok_or_else(|| Error::Config("NANELO_TOKEN required".into()))?.clone();
        Ok(Box::new(Nanelo { token }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let fulldomain = name;
        let (zone, sub_domain) = self.resolve_zone(fulldomain)
            .map_err(|e| Error::Provider(format!("nanelo add TXT: {e}")))?;
        let url = format!(
            "{BASE_URL}{}/dns/addrecord?domain={}&type=TXT&ttl=60&name={}&value={}",
            self.token, zone, sub_domain, value
        );
        let resp = http::post(&url, b"", "application/x-www-form-urlencoded", &[])
            .map_err(|e| Error::Provider(format!("nanelo add TXT: {e}")))?;
        if resp.status >= 400 || !resp.body.contains("success") {
            return Err(Error::Provider(format!("nanelo add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let fulldomain = name;
        let (zone, sub_domain) = match self.resolve_zone(fulldomain) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let url = format!(
            "{BASE_URL}{}/dns/deleterecord?domain={}&type=TXT&ttl=60&name={}&value={}",
            self.token, zone, sub_domain, value
        );
        let resp = match http::post(&url, b"", "application/x-www-form-urlencoded", &[]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if resp.status >= 400 || !resp.body.contains("success") {
            eprintln!("warning: nanelo remove TXT: HTTP {} {}", resp.status, resp.body);
        }
        Ok(())
    }
}
