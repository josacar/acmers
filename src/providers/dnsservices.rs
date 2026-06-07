use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Dnsservices {
    key: String,
    secret: String,
}

impl DnsProvider for Dnsservices {
    fn slug() -> &'static str {
        "dnsservices"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DNSServices_Key", "DNSServices_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let key = env.get("DNSServices_Key")
            .ok_or_else(|| Error::Config("DNSServices_Key required".into()))?
            .clone();
        let secret = env.get("DNSServices_Secret")
            .ok_or_else(|| Error::Config("DNSServices_Secret required".into()))?
            .clone();
        Ok(Box::new(Dnsservices { key, secret }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let auth_params = format!("apikey={}&secret={}", self.key, self.secret);
        let body = serde_json::json!({
            "type": "TXT",
            "name": name,
            "content": value,
            "ttl": 120,
        });
        let url = format!("https://dns.services/api/dns/zones/{zone_id}/records?{auth_params}");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("dnsservices add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("dnsservices add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = match self.resolve_zone(domain) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let auth_params = format!("apikey={}&secret={}", self.key, self.secret);
        let list_url = format!("https://dns.services/api/dns/zones/{zone_id}/records?{auth_params}");
        let resp = match http::get(&list_url, &[]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(records) = v.as_array() {
            for r in records {
                if r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && r.get("name").and_then(|n| n.as_str()) == Some(name)
                    && r.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = r.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://dns.services/api/dns/zones/{zone_id}/records/{id}?{auth_params}");
                        let _ = http::delete(&del_url, &[]);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Dnsservices {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let url = format!("https://dns.services/api/dns/zones?apikey={}&secret={}", self.key, self.secret);
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("dnsservices list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("dnsservices parse zones: {e}")))?;
        if let Some(arr) = v.as_array() {
            for z in arr {
                if let Some(nm) = z.get("name").or_else(|| z.get("domain")).and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        if let Some(id) = z.get("id").and_then(|i| {
                            if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                        }) {
                            return Ok(id);
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
