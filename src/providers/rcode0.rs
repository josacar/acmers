use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const DEFAULT_URL: &str = "https://my.rcodezero.at";
const DEFAULT_TTL: u32 = 60;

pub struct Rcode0 {
    token: String,
    base_url: String,
    ttl: u32,
}

impl DnsProvider for Rcode0 {
    fn slug() -> &'static str {
        "rcode0"
    }

    fn env_vars() -> &'static [&'static str] {
        &["RCODE0_API_TOKEN", "RCODE0_URL", "RCODE0_TTL"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("RCODE0_API_TOKEN")
            .ok_or_else(|| Error::Config("RCODE0_API_TOKEN required".into()))?
            .clone();
        let base_url = env.get("RCODE0_URL")
            .map(|s| s.trim_end_matches('/').to_string())
            .unwrap_or_else(|| DEFAULT_URL.to_string());
        let ttl = env.get("RCODE0_TTL")
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_TTL);
        Ok(Box::new(Rcode0 { token, base_url, ttl }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let zone = self.resolve_zone(name, headers)?;
        let existing = self.list_existing_challenges(&zone, name, headers)?;

        let mut records = vec![self.build_record(value)];
        for old in &existing {
            records.push(self.build_record(old));
        }

        let changetype = if existing.is_empty() { "add" } else { "update" };
        let body = serde_json::to_vec(&serde_json::json!([{
            "changetype": changetype,
            "name": format!("{name}."),
            "type": "TXT",
            "ttl": self.ttl,
            "records": records,
        }])).unwrap();

        let url = format!("{}/api/v1/acme/zones/{}/rrsets", self.base_url, zone);
        let resp = http::patch(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("rcode0 add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("rcode0 add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let zone = match self.resolve_zone(name, headers) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let existing = match self.list_existing_challenges(&zone, name, headers) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        if !existing.iter().any(|c| c == value) {
            return Ok(());
        }

        let del_body = serde_json::to_vec(&serde_json::json!([{
            "changetype": "delete",
            "name": format!("{name}."),
            "type": "TXT",
        }])).unwrap();
        let url = format!("{}/api/v1/acme/zones/{}/rrsets", self.base_url, zone);
        let resp = http::patch(&url, &del_body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("rcode0 del TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("rcode0 del TXT: HTTP {} {}", resp.status, resp.body)));
        }

        let remaining: Vec<&String> = existing.iter().filter(|c| c.as_str() != value).collect();
        if !remaining.is_empty() {
            let mut records = Vec::new();
            for old in &remaining {
                records.push(self.build_record(old));
            }
            let upd_body = serde_json::to_vec(&serde_json::json!([{
                "changetype": "update",
                "name": format!("{name}."),
                "type": "TXT",
                "ttl": self.ttl,
                "records": records,
            }])).unwrap();
            let resp = http::patch(&url, &upd_body, "application/json", headers)
                .map_err(|e| Error::Provider(format!("rcode0 upd TXT: {e}")))?;
            if resp.status >= 400 {
                return Err(Error::Provider(format!("rcode0 upd TXT: HTTP {} {}", resp.status, resp.body)));
            }
        }
        Ok(())
    }
}

impl Rcode0 {
    fn build_record(&self, value: &str) -> Value {
        serde_json::json!({
            "content": format!("\"{}\"", value),
            "disabled": false,
        })
    }

    fn resolve_zone(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            let url = format!("{}/api/v1/acme/zones/{}", self.base_url, h);
            let resp = match http::get(&url, headers) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let body = resp.body.trim();
            if body == "[\"found\"]" {
                return Ok(h);
            }
            if body == "[\"not a master domain\"]" {
                return Err(Error::Provider(format!("rcode0: not a master domain: {h}")));
            }
        }
        Err(Error::Provider(format!("rcode0: zone not found for {domain}")))
    }

    fn list_existing_challenges(&self, zone: &str, fulldomain: &str, headers: &[(&str, &str)]) -> Result<Vec<String>, Error> {
        let url = format!("{}/api/v1/acme/zones/{}/rrsets", self.base_url, zone);
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("rcode0 list rrsets: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("rcode0 rrsets: {e}")))?;
        let mut challenges = Vec::new();
        if let Some(rrsets) = v.as_array() {
            for rrset in rrsets {
                if rrset.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && rrset.get("name").and_then(|n| n.as_str()) == Some(&format!("{fulldomain}."))
                {
                    if let Some(records) = rrset.get("records").and_then(|r| r.as_array()) {
                        for rec in records {
                            if let Some(content) = rec.get("content").and_then(|c| c.as_str()) {
                                let trimmed = content.trim_matches('"');
                                if !trimmed.is_empty() {
                                    challenges.push(trimmed.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(challenges)
    }
}
