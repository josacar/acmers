use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Cloudflare {
    token: String,
    zone_id: Option<String>,
    account_id: Option<String>,
}

impl DnsProvider for Cloudflare {
    fn slug() -> &'static str {
        "cf"
    }

    fn env_vars() -> &'static [&'static str] {
        &["CF_Token", "CF_Zone_ID", "CF_Account_ID"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("CF_Token").ok_or_else(|| {
            Error::Config("CF_Token environment variable required".into())
        })?.clone();
        Ok(Box::new(Cloudflare {
            token,
            zone_id: env.get("CF_Zone_ID").cloned(),
            account_id: env.get("CF_Account_ID").cloned(),
        }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let record = serde_json::json!({
            "type": "TXT",
            "name": name,
            "content": value,
            "ttl": 120,
        });
        let url = format!("https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records");
        let body = serde_json::to_vec(&record).unwrap();
        let auth = format!("Bearer {}", self.token);
        let resp = http::post(&url, &body, "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("CF add TXT: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("CF response: {e}")))?;
        if v.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
            Ok(())
        } else {
            let err = v.get("errors").and_then(|e| e.as_array())
                .and_then(|a| a.first())
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("unknown");
            Err(Error::Provider(format!("CF add TXT: {err}")))
        }
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let auth = format!("Bearer {}", self.token);
        let list_url = format!(
            "https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records?type=TXT&name={name}&content={value}"
        );
        let resp = http::get(&list_url, &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("CF list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("CF list response: {e}")))?;
        if let Some(records) = v.get("result").and_then(|r| r.as_array()) {
            for record in records {
                if let Some(id) = record.get("id").and_then(|i| i.as_str()) {
                    let del_url = format!(
                        "https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records/{id}"
                    );
                    let body = serde_json::json!({});
                    let _ = http::post(&del_url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)]);
                }
            }
        }
        Ok(())
    }
}

impl Cloudflare {

    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        if let Some(ref zid) = self.zone_id {
            return Ok(zid.clone());
        }

        let auth = format!("Bearer {}", self.token);

        if let Some(ref account_id) = self.account_id {
            let list_url = format!("https://api.cloudflare.com/client/v4/zones?account.id={account_id}");
            let resp = http::get(&list_url, &[("Authorization", &auth)])
                .map_err(|e| Error::Provider(format!("CF list zones: {e}")))?;
            let v: Value = serde_json::from_str(&resp.body)
                .map_err(|e| Error::Json(format!("CF zones response: {e}")))?;
            if let Some(zones) = v.get("result").and_then(|r| r.as_array()) {
                for z in zones {
                    if let Some(name) = z.get("name").and_then(|n| n.as_str()) {
                        if domain == name || domain.ends_with(&format!(".{name}")) {
                            if let Some(id) = z.get("id").and_then(|i| i.as_str()) {
                                return Ok(id.to_string());
                            }
                        }
                    }
                }
            }
        }

        let mut search = domain.to_string();
        loop {
            let search_url = format!("https://api.cloudflare.com/client/v4/zones?name={search}");
            let resp = http::get(&search_url, &[("Authorization", &auth)])
                .map_err(|e| Error::Provider(format!("CF search zones: {e}")))?;
            let v: Value = serde_json::from_str(&resp.body)
                .map_err(|e| Error::Json(format!("CF zones response: {e}")))?;
            if let Some(zones) = v.get("result").and_then(|r| r.as_array()) {
                if let Some(z) = zones.first() {
                    if let Some(id) = z.get("id").and_then(|i| i.as_str()) {
                        return Ok(id.to_string());
                    }
                }
            }
            if let Some(pos) = search.find('.') {
                search = search[pos + 1..].to_string();
            } else {
                break;
            }
        }

        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
