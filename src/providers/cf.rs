use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Cloudflare {
    token: Option<String>,
    key: Option<String>,
    email: Option<String>,
    zone_id: Option<String>,
    account_id: Option<String>,
}

impl DnsProvider for Cloudflare {
    fn slug() -> &'static str {
        "cf"
    }

    fn env_vars() -> &'static [&'static str] {
        &["CF_Token", "CF_Key", "CF_Email", "CF_Zone_ID", "CF_Account_ID"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("CF_Token").cloned();
        let key = env.get("CF_Key").cloned();
        let email = env.get("CF_Email").cloned();

        if token.is_none() && (key.is_none() || email.is_none()) {
            return Err(Error::Config("CF_Token or CF_Key+CF_Email required".into()));
        }

        Ok(Box::new(Cloudflare {
            token,
            key,
            email,
            zone_id: env.get("CF_Zone_ID").cloned(),
            account_id: env.get("CF_Account_ID").cloned(),
        }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let quoted_value = format!("\"{}\"", value);
        let record = serde_json::json!({
            "type": "TXT",
            "name": name,
            "content": quoted_value,
            "ttl": 120,
        });
        let url = format!("https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records");
        let body = serde_json::to_vec(&record).unwrap();
        let headers = self.auth_headers();
        let refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let resp = http::post(&url, &body, "application/json", &refs)
            .map_err(|e| Error::Provider(format!("CF add TXT: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("CF response: {e}")))?;
        if v.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
            return Ok(());
        }
        if let Some(errors) = v.get("errors").and_then(|e| e.as_array()) {
            for err in errors {
                let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("");
                if msg.contains("already exists") || msg.contains("identical")
                    || err.get("code").and_then(|c| c.as_u64()) == Some(81058)
                {
                    return Ok(());
                }
            }
            if let Some(first) = errors.first() {
                let msg = first.get("message").and_then(|m| m.as_str()).unwrap_or("unknown");
                return Err(Error::Provider(format!("CF add TXT: {msg}")));
            }
        }
        Err(Error::Provider("CF add TXT: unknown error".into()))
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let headers = self.auth_headers();
        let refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let quoted_value = format!("\"{}\"", value);
        let list_url = format!(
            "https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records?type=TXT&name={name}&content={quoted_value}"
        );
        let resp = http::get(&list_url, &refs)
            .map_err(|e| Error::Provider(format!("CF list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("CF list response: {e}")))?;
        if let Some(records) = v.get("result").and_then(|r| r.as_array()) {
            for record in records {
                if let Some(id) = record.get("id").and_then(|i| i.as_str()) {
                    let del_url = format!(
                        "https://api.cloudflare.com/client/v4/zones/{zone_id}/dns_records/{id}"
                    );
                    let _ = http::delete(&del_url, &refs);
                }
            }
        }
        Ok(())
    }
}

impl Cloudflare {
    fn auth_headers(&self) -> Vec<(String, String)> {
        if let Some(ref token) = self.token {
            vec![("Authorization".to_string(), format!("Bearer {token}"))]
        } else {
            vec![
                ("X-Auth-Email".to_string(), self.email.clone().unwrap_or_default()),
                ("X-Auth-Key".to_string(), self.key.clone().unwrap_or_default()),
            ]
        }
    }

    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        if let Some(ref zid) = self.zone_id {
            let url = format!("https://api.cloudflare.com/client/v4/zones/{zid}");
            let headers = self.auth_headers();
            let refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            let resp = http::get(&url, &refs)
                .map_err(|e| Error::Provider(format!("CF get zone: {e}")))?;
            let v: Value = serde_json::from_str(&resp.body)
                .map_err(|e| Error::Json(format!("CF zone response: {e}")))?;
            if v.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
                return Ok(zid.clone());
            }
            return Err(Error::Provider(format!("invalid zone ID {zid}")));
        }

        let headers = self.auth_headers();
        let refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

        let mut search = domain.to_string();
        loop {
            let search_url = if let Some(ref account_id) = self.account_id {
                format!("https://api.cloudflare.com/client/v4/zones?name={search}&account.id={account_id}")
            } else {
                format!("https://api.cloudflare.com/client/v4/zones?name={search}")
            };
            let resp = http::get(&search_url, &refs)
                .map_err(|e| Error::Provider(format!("CF search zones: {e}")))?;
            let v: Value = serde_json::from_str(&resp.body)
                .map_err(|e| Error::Json(format!("CF zones response: {e}")))?;
            if let Some(zones) = v.get("result").and_then(|r| r.as_array()) {
                if let Some(z) = zones.first() {
                    if let Some(name) = z.get("name").and_then(|n| n.as_str()) {
                        if name == search {
                            if let Some(id) = z.get("id").and_then(|i| i.as_str()) {
                                return Ok(id.to_string());
                            }
                        }
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
