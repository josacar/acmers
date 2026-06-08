use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Rackspace {
    username: String,
    api_key: String,
}

impl DnsProvider for Rackspace {
    fn slug() -> &'static str {
        "rackspace"
    }

    fn env_vars() -> &'static [&'static str] {
        &["RACKSPACE_Username", "RACKSPACE_ApiKey"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("RACKSPACE_Username")
            .ok_or_else(|| Error::Config("RACKSPACE_Username required".into()))?.clone();
        let api_key = env.get("RACKSPACE_ApiKey")
            .ok_or_else(|| Error::Config("RACKSPACE_ApiKey required".into()))?.clone();
        Ok(Box::new(Rackspace { username, api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let (token, tenant_id) = self.get_token()?;
        let headers: &[(&str, &str)] = &[("X-Auth-Token", &token), ("Accept", "application/json")];
        let base = format!("https://dns.api.rackspacecloud.com/v1.0/{}", tenant_id);
        let domain_id = self.resolve_zone(domain, &base, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "name": name,
            "type": "TXT",
            "data": value,
            "ttl": 120,
        })).unwrap();
        let url = format!("{}/domains/{}/records", base, domain_id);
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("Rackspace add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Rackspace add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let (token, tenant_id) = match self.get_token() {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };
        let headers: &[(&str, &str)] = &[("X-Auth-Token", &token), ("Accept", "application/json")];
        let base = format!("https://dns.api.rackspacecloud.com/v1.0/{}", tenant_id);
        let domain_id = match self.resolve_zone(domain, &base, headers) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let list_url = format!("{}/domains/{}/records", base, domain_id);
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(records) = v.get("records").and_then(|r| r.as_array()) {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("data").and_then(|d| d.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| i.as_str()) {
                        let del_url = format!("{}/domains/{}/records/{}", base, domain_id, id);
                        let _ = http::delete(&del_url, headers);
                    }
                    return Ok(());
                }
            }
        }
        Ok(())
    }
}

impl Rackspace {
    fn get_token(&self) -> Result<(String, String), Error> {
        let body = serde_json::to_vec(&serde_json::json!({
            "auth": {
                "RAX-KSKEY:apiKeyCredentials": {
                    "username": self.username,
                    "apiKey": self.api_key,
                }
            }
        })).unwrap();
        let resp = http::post(
            "https://identity.api.rackspacecloud.com/v2.0/tokens",
            &body,
            "application/json",
            &[]
        ).map_err(|e| Error::Provider(format!("Rackspace auth: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Rackspace auth: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Rackspace auth: {e}")))?;
        let token = crate::json::get_string_required(&v, &["access", "token", "id"])?.to_string();
        let tenant = crate::json::get_string_required(&v, &["access", "token", "tenant", "id"])?.to_string();
        Ok((token, tenant))
    }

    fn resolve_zone(&self, domain: &str, base: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let url = format!("{}/domains", base);
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("Rackspace list domains: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Rackspace list domains: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Rackspace domains: {e}")))?;
        if let Some(domains) = v.get("domains").and_then(|d| d.as_array()) {
            for d in domains {
                if let Some(nm) = d.get("name").and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        if let Some(id) = d.get("id").and_then(|i| i.as_i64()) {
                            return Ok(id.to_string());
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
