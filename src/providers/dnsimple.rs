use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Dnsimple {
    token: String,
}

impl DnsProvider for Dnsimple {
    fn slug() -> &'static str {
        "dnsimple"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DNSimple_OAUTH_TOKEN"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("DNSimple_OAUTH_TOKEN")
            .ok_or_else(|| Error::Config("DNSimple_OAUTH_TOKEN required".into()))?
            .clone();
        Ok(Box::new(Dnsimple { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let account_id = self.resolve_account(headers)?;
        let zone_name = self.resolve_zone(domain, &account_id, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "name": name,
            "content": value,
            "ttl": 120,
        })).unwrap();
        let url = format!("https://api.dnsimple.com/v2/{account_id}/zones/{zone_name}/records");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("DNSimple add TXT: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("DNSimple response: {e}")))?;
        if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
            if resp.status >= 400 {
                return Err(Error::Provider(format!("DNSimple add TXT: {msg}")));
            }
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let account_id = match self.resolve_account(headers) {
            Ok(a) => a,
            Err(_) => return Ok(()),
        };
        let zone_name = match self.resolve_zone(domain, &account_id, headers) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let list_url = format!("https://api.dnsimple.com/v2/{account_id}/zones/{zone_name}/records?type=TXT");
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(records) = v.get("data").and_then(|r| r.as_array()) {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| i.as_i64()) {
                        let del_url = format!("https://api.dnsimple.com/v2/{account_id}/zones/{zone_name}/records/{id}");
                        let _ = http::delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Dnsimple {
    fn resolve_account(&self, headers: &[(&str, &str)]) -> Result<String, Error> {
        let resp = http::get("https://api.dnsimple.com/v2/accounts", headers)
            .map_err(|e| Error::Provider(format!("DNSimple list accounts: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("DNSimple accounts: {e}")))?;
        if let Some(accounts) = v.get("data").and_then(|d| d.as_array()) {
            if let Some(acc) = accounts.first() {
                if let Some(id) = acc.get("id").and_then(|i| i.as_i64()) {
                    return Ok(id.to_string());
                }
            }
        }
        Err(Error::Provider("DNSimple account not found".into()))
    }

    fn resolve_zone(&self, domain: &str, account_id: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let url = format!("https://api.dnsimple.com/v2/{account_id}/zones");
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("DNSimple list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("DNSimple zones: {e}")))?;
        if let Some(zones) = v.get("data").and_then(|d| d.as_array()) {
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
