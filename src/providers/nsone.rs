use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Nsone {
    api_key: String,
}

impl DnsProvider for Nsone {
    fn slug() -> &'static str {
        "nsone"
    }

    fn env_vars() -> &'static [&'static str] {
        &["NS1_Key"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("NS1_Key")
            .ok_or_else(|| Error::Config("NS1_Key required".into()))?
            .clone();
        Ok(Box::new(Nsone { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        self.resolve_zone(domain)?;
        let record_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let headers: &[(&str, &str)] = &[("X-NSONE-Key", &self.api_key)];
        let url = format!("https://api.nsone.net/v1/zones/{domain}/{domain}/{record_name}/TXT");
        let body = serde_json::json!({
            "answers": [{"answer": [value]}],
            "ttl": 60,
        });
        http::put(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("nsone add TXT: {e}")))?;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        if self.resolve_zone(domain).is_err() {
            return Ok(());
        }
        let record_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let headers: &[(&str, &str)] = &[("X-NSONE-Key", &self.api_key)];
        let url = format!("https://api.nsone.net/v1/zones/{domain}/{domain}/{record_name}/TXT");
        let _ = http::delete(&url, headers);
        Ok(())
    }
}

impl Nsone {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let headers: &[(&str, &str)] = &[("X-NSONE-Key", &self.api_key)];
        let resp = http::get("https://api.nsone.net/v1/zones", headers)
            .map_err(|e| Error::Provider(format!("nsone list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("nsone parse zones: {e}")))?;
        if let Some(arr) = v.as_array() {
            for z in arr {
                if let Some(nm) = z.get("zone").and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        return Ok(nm.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
