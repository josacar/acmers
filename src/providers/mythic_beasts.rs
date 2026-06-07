use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct MythicBeasts {
    basic_auth: String,
}

impl DnsProvider for MythicBeasts {
    fn slug() -> &'static str {
        "mythic_beasts"
    }

    fn env_vars() -> &'static [&'static str] {
        &["MYTHIC_BEASTS_Key", "MYTHIC_BEASTS_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let key = env.get("MYTHIC_BEASTS_Key")
            .ok_or_else(|| Error::Config("MYTHIC_BEASTS_Key required".into()))?
            .clone();
        let secret = env.get("MYTHIC_BEASTS_Secret")
            .ok_or_else(|| Error::Config("MYTHIC_BEASTS_Secret required".into()))?
            .clone();
        let creds = format!("{key}:{secret}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(MythicBeasts { basic_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let zone = self.resolve_zone(domain, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "data": [value],
            "ttl": 120,
        })).unwrap();
        let url = format!("https://api.mythic-beasts.com/dns/v2/zones/{zone}/records/{name}/TXT");
        let resp = http::put(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("MythicBeasts add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("MythicBeasts add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let zone = match self.resolve_zone(domain, headers) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let url = format!("https://api.mythic-beasts.com/dns/v2/zones/{zone}/records/{name}/TXT");
        let _ = http::delete(&url, headers);
        Ok(())
    }
}

impl MythicBeasts {
    fn resolve_zone(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let resp = http::get("https://api.mythic-beasts.com/dns/v2/zones", headers)
            .map_err(|e| Error::Provider(format!("MythicBeasts list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("MythicBeasts zones: {e}")))?;
        if let Some(zones) = v.as_array() {
            for z in zones {
                if let Some(name) = z.get("name").and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        return Ok(name.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
