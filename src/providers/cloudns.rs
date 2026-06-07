use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Cloudns {
    auth_id: String,
    auth_password: String,
}

impl DnsProvider for Cloudns {
    fn slug() -> &'static str {
        "cloudns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["CLOUDNS_AUTH_ID", "CLOUDNS_AUTH_PASSWORD"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let auth_id = env.get("CLOUDNS_AUTH_ID")
            .ok_or_else(|| Error::Config("CLOUDNS_AUTH_ID required".into()))?.clone();
        let auth_password = env.get("CLOUDNS_AUTH_PASSWORD")
            .ok_or_else(|| Error::Config("CLOUDNS_AUTH_PASSWORD required".into()))?.clone();
        Ok(Box::new(Cloudns { auth_id, auth_password }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let _ = self.resolve_zone(domain)?;
        let url = format!(
            "https://api.cloudns.net/dns/add-record.json?auth-id={}&auth-password={}&domain-name={}&record-type=TXT&host={}&record={}&ttl=120",
            self.auth_id, self.auth_password, domain, name, value
        );
        let resp = http::post(&url, b"", "application/json", &[])
            .map_err(|e| Error::Provider(format!("cloudns add TXT: {e}")))?;
        if resp.status != 200 {
            return Err(Error::Provider(format!("cloudns add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let _ = self.resolve_zone(domain)?;
        let list_url = format!(
            "https://api.cloudns.net/dns/records.json?auth-id={}&auth-password={}&domain-name={}&type=TXT&host={}",
            self.auth_id, self.auth_password, domain, name
        );
        let resp = http::post(&list_url, b"", "application/json", &[])
            .map_err(|e| Error::Provider(format!("cloudns list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("cloudns records: {e}")))?;
        if let Some(records) = v.as_object().and_then(|o| o.values().next()).and_then(|r| r.as_array())
            .or_else(|| v.as_array())
        {
            for record in records {
                if let (Some(t), Some(rv)) = (record.get("type").and_then(|t| t.as_str()), record.get("record").and_then(|r| r.as_str())) {
                    if t == "TXT" && rv == value {
                        if let Some(id) = record.get("id").and_then(|i| i.as_str()) {
                            let del_url = format!(
                                "https://api.cloudns.net/dns/delete-record.json?auth-id={}&auth-password={}&domain-name={}&record-id={}",
                                self.auth_id, self.auth_password, domain, id
                            );
                            http::post(&del_url, b"", "application/json", &[])
                                .map_err(|e| Error::Provider(format!("cloudns delete TXT: {e}")))?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl Cloudns {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let url = format!(
            "https://api.cloudns.net/dns/list-zones.json?page=1&rows-per-page=100&auth-id={}&auth-password={}",
            self.auth_id, self.auth_password
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("cloudns list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("cloudns zones: {e}")))?;
        if let Some(arr) = v.as_array() {
            for z in arr {
                if let Some(nm) = z.get("name").and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        return Ok(nm.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
