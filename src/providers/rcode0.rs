use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Rcode0 {
    token: String,
}

impl DnsProvider for Rcode0 {
    fn slug() -> &'static str {
        "rcode0"
    }

    fn env_vars() -> &'static [&'static str] {
        &["RCODE0_Token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("RCODE0_Token")
            .ok_or_else(|| Error::Config("RCODE0_Token required".into()))?
            .clone();
        Ok(Box::new(Rcode0 { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &format!("Bearer {}", self.token))];
        let zone = self.resolve_zone(domain, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "name": name,
            "type": "TXT",
            "ttl": 120,
            "records": [{"content": value}],
        })).unwrap();
        let url = format!("https://api.rcodezero.at/v1/zones/{zone}/rrsets");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("rcode0 add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("rcode0 add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &format!("Bearer {}", self.token))];
        let zone = match self.resolve_zone(domain, headers) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let list_url = format!("https://api.rcodezero.at/v1/zones/{zone}/rrsets");
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(rrsets) = v.as_array() {
            for rrset in rrsets {
                if rrset.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && rrset.get("name").and_then(|n| n.as_str()) == Some(name)
                {
                    let del_url = format!("https://api.rcodezero.at/v1/zones/{zone}/rrsets/{name}/TXT");
                    let _ = http::delete(&del_url, headers);
                    return Ok(());
                }
            }
        }
        Ok(())
    }
}

impl Rcode0 {
    fn resolve_zone(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let resp = http::get("https://api.rcodezero.at/v1/zones", headers)
            .map_err(|e| Error::Provider(format!("rcode0 list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("rcode0 zones: {e}")))?;
        if let Some(zones) = v.as_array() {
            for zone in zones {
                if let Some(nm) = zone.get("name").and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        return Ok(nm.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
