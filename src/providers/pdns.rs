use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Powerdns {
    token: String,
    base_url: String,
    server_id: String,
    ttl: u32,
}

impl DnsProvider for Powerdns {
    fn slug() -> &'static str {
        "pdns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["PDNS_Url", "PDNS_ServerId", "PDNS_Token", "PDNS_Ttl"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let base_url = env.get("PDNS_Url")
            .ok_or_else(|| Error::Config("PDNS_Url required".into()))?.clone();
        let server_id = env.get("PDNS_ServerId")
            .map(|s| s.clone()).unwrap_or_else(|| "localhost".to_string());
        let token = env.get("PDNS_Token")
            .ok_or_else(|| Error::Config("PDNS_Token required".into()))?.clone();
        let ttl = env.get("PDNS_Ttl")
            .and_then(|t| t.parse().ok()).unwrap_or(60);
        Ok(Box::new(Powerdns { token, base_url, server_id, ttl }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_url = self.resolve_zone(domain)?;
        let record_name = format!("{name}.{domain}.");
        let body = serde_json::json!({
            "rrsets": [{
                "name": record_name,
                "type": "TXT",
                "ttl": self.ttl,
                "changetype": "REPLACE",
                "records": [{
                    "content": format!("\"{value}\""),
                    "disabled": false,
                }],
            }],
        });
        let resp = http::patch(&zone_url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("X-API-Key", &self.token)])
            .map_err(|e| Error::Provider(format!("pdns add TXT: {e}")))?;
        if resp.status >= 300 {
            return Err(Error::Provider(format!("pdns add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let zone_url = match self.resolve_zone(domain) {
            Ok(url) => url,
            Err(e) => {
                eprintln!("warning: pdns cleanup zone not found: {e}");
                return Ok(());
            }
        };
        let record_name = format!("{name}.{domain}.");
        let body = serde_json::json!({
            "rrsets": [{
                "name": record_name,
                "type": "TXT",
                "ttl": self.ttl,
                "changetype": "DELETE",
            }],
        });
        http::patch(&zone_url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("X-API-Key", &self.token)]).ok();
        Ok(())
    }
}

impl Powerdns {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let url = format!("{}/servers/{}/zones", self.base_url, self.server_id);
        let resp = http::get(&url, &[("X-API-Key", &self.token)])
            .map_err(|e| Error::Provider(format!("pdns list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("pdns zones: {e}")))?;
        if let Some(zones) = v.as_array() {
            for z in zones {
                if let Some(zname) = z.get("name").and_then(|n| n.as_str()) {
                    let zname_clean = zname.trim_end_matches('.');
                    if domain == zname_clean || domain.ends_with(&format!(".{zname_clean}")) {
                        if let Some(zone_url) = z.get("url").and_then(|u| u.as_str()) {
                            return Ok(zone_url.to_string());
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
