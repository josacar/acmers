use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct World4you {
    auth_token: String,
}

impl DnsProvider for World4you {
    fn slug() -> &'static str {
        "world4you"
    }

    fn env_vars() -> &'static [&'static str] {
        &["WORLD4YOU_Username", "WORLD4YOU_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("WORLD4YOU_Username")
            .ok_or_else(|| Error::Config("WORLD4YOU_Username required".into()))?.clone();
        let password = env.get("WORLD4YOU_Password")
            .ok_or_else(|| Error::Config("WORLD4YOU_Password required".into()))?.clone();
        let creds = format!("{username}:{password}");
        let auth_token = base64::encode_std(creds.as_bytes());
        Ok(Box::new(World4you { auth_token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let _ = self.resolve_zone(domain)?;
        let url = format!("https://my.world4you.com/api/v1/domains/{domain}/dnsrecords");
        let body = serde_json::json!({
            "name": name,
            "type": "TXT",
            "value": value,
            "ttl": 120,
        });
        http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("AuthToken", &self.auth_token)])
            .map_err(|e| Error::Provider(format!("world4you add TXT: {e}")))?;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let _ = self.resolve_zone(domain)?;
        let record_id = self.find_record_id(domain, name, value)?;
        if let Some(id) = record_id {
            let url = format!("https://my.world4you.com/api/v1/domains/{domain}/dnsrecords/{id}");
            http::delete(&url, &[("AuthToken", &self.auth_token)])
                .map_err(|e| Error::Provider(format!("world4you delete TXT: {e}")))?;
        }
        Ok(())
    }
}

impl World4you {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let resp = http::get("https://my.world4you.com/api/v1/domains", &[("AuthToken", &self.auth_token)])
            .map_err(|e| Error::Provider(format!("world4you list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("world4you domains: {e}")))?;
        if let Some(arr) = v.as_array() {
            for d in arr {
                if let Some(nm) = d.get("domain").or_else(|| d.get("name")).and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        return Ok(nm.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }

    fn find_record_id(&self, domain: &str, name: &str, value: &str) -> Result<Option<String>, Error> {
        let url = format!("https://my.world4you.com/api/v1/domains/{domain}/dnsrecords");
        let resp = http::get(&url, &[("AuthToken", &self.auth_token)])
            .map_err(|e| Error::Provider(format!("world4you list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("world4you records: {e}")))?;
        if let Some(arr) = v.as_array() {
            for r in arr {
                if r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && r.get("name").and_then(|n| n.as_str()) == Some(name)
                    && r.get("value").and_then(|n| n.as_str()) == Some(value)
                {
                    return Ok(value_to_string(r.get("id")));
                }
            }
        }
        Ok(None)
    }
}

fn value_to_string(v: Option<&Value>) -> Option<String> {
    v.and_then(|v| v.as_str().map(|s| s.to_string())
        .or_else(|| v.as_i64().map(|i| i.to_string())))
}
