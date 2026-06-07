use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Conoha {
    username: String,
    password: String,
    tenant_id: String,
    identity_api: String,
}

impl DnsProvider for Conoha {
    fn slug() -> &'static str {
        "conoha"
    }

    fn env_vars() -> &'static [&'static str] {
        &["CONOHA_Username", "CONOHA_Password", "CONOHA_TenantId", "CONOHA_IdentityServiceApi"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("CONOHA_Username")
            .ok_or_else(|| Error::Config("CONOHA_Username required".into()))?
            .clone();
        let password = env.get("CONOHA_Password")
            .ok_or_else(|| Error::Config("CONOHA_Password required".into()))?
            .clone();
        let tenant_id = env.get("CONOHA_TenantId")
            .ok_or_else(|| Error::Config("CONOHA_TenantId required".into()))?
            .clone();
        let identity_api = env.get("CONOHA_IdentityServiceApi")
            .cloned()
            .unwrap_or_else(|| "https://identity.tyo1.conoha.io/v2.0".to_string());
        Ok(Box::new(Conoha { username, password, tenant_id, identity_api }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = self.get_token()?;
        let headers: &[(&str, &str)] = &[("X-Auth-Token", &token)];
        let domain_id = self.resolve_zone(domain, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "name": name,
            "type": "TXT",
            "data": value,
            "ttl": 120,
        })).unwrap();
        let url = format!("https://dns-service.tyo1.conoha.io/v1/{}/domains/{}/records", self.tenant_id, domain_id);
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("Conoha add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Conoha add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = match self.get_token() {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };
        let headers: &[(&str, &str)] = &[("X-Auth-Token", &token)];
        let domain_id = match self.resolve_zone(domain, headers) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };
        let list_url = format!("https://dns-service.tyo1.conoha.io/v1/{}/domains/{}/records", self.tenant_id, domain_id);
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
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://dns-service.tyo1.conoha.io/v1/{}/domains/{}/records/{id}", self.tenant_id, domain_id);
                        let _ = http::delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Conoha {
    fn get_token(&self) -> Result<String, Error> {
        let body = serde_json::to_vec(&serde_json::json!({
            "auth": {
                "passwordCredentials": {
                    "username": self.username,
                    "password": self.password,
                },
                "tenantId": self.tenant_id,
            }
        })).unwrap();
        let url = format!("{}/tokens", self.identity_api);
        let resp = http::post(&url, &body, "application/json", &[])
            .map_err(|e| Error::Provider(format!("Conoha auth: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Conoha auth: {e}")))?;
        v.pointer("/access/token/id").and_then(|i| i.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| Error::Provider(format!("Conoha auth: no token in response: {}", resp.body)))
    }

    fn resolve_zone(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let url = format!("https://dns-service.tyo1.conoha.io/v1/{}/domains", self.tenant_id);
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("Conoha list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Conoha zones: {e}")))?;
        if let Some(domains) = v.get("domains").and_then(|d| d.as_array()) {
            for d in domains {
                if d.get("name").and_then(|n| n.as_str()) == Some(domain) {
                    if let Some(id) = d.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        return Ok(id);
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
