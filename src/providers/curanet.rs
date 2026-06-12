use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::json as j;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Curanet {
    client_id: String,
    client_secret: String,
}

impl DnsProvider for Curanet {
    fn slug() -> &'static str {
        "curanet"
    }

    fn env_vars() -> &'static [&'static str] {
        &["CURANET_AUTH_CLIENT_ID", "CURANET_AUTH_CLIENT_SECRET"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let client_id = env.get("CURANET_AUTH_CLIENT_ID")
            .ok_or_else(|| Error::Config("CURANET_AUTH_CLIENT_ID required".into()))?
            .clone();
        let client_secret = env.get("CURANET_AUTH_CLIENT_SECRET")
            .ok_or_else(|| Error::Config("CURANET_AUTH_CLIENT_SECRET required".into()))?
            .clone();
        Ok(Box::new(Curanet { client_id, client_secret }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = self.get_token()?;
        let zone = self.get_root(domain, &token)?;
        let url = format!("https://api.curanet.dk/dns/v1/Domains/{zone}/Records");
        let body = serde_json::to_vec(&serde_json::json!({
            "name": name,
            "type": "TXT",
            "ttl": 60,
            "priority": 0,
            "data": value,
        })).unwrap();
        let auth = format!("Bearer {token}");
        let resp = http::post(&url, &body, "application/json",
            &[("Authorization", &auth), ("Accept", "application/json")])
            .map_err(|e| Error::Provider(format!("Curanet add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Curanet add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        if !resp.body.contains(value) {
            return Err(Error::Provider(format!("Curanet add TXT: unexpected response: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = match self.get_token() {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };
        let zone = match self.get_root(domain, &token) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let url = format!("https://api.curanet.dk/dns/v1/Domains/{zone}/Records");
        let auth = format!("Bearer {token}");
        let resp = match http::get(&url, &[("Authorization", &auth), ("Accept", "application/json")]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records = v.as_array();
        if let Some(arr) = records {
            for record in arr {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("data").and_then(|d| d.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("https://api.curanet.dk/dns/v1/Domains/{zone}/Records/{id}");
                        let _ = http::delete(&del_url, &[("Authorization", &auth)]);
                    }
                }
            }
        }
        Ok(())
    }
}

impl Curanet {
    fn get_token(&self) -> Result<String, Error> {
        let data = format!(
            "grant_type=client_credentials&client_id={}&client_secret={}&scope=dns",
            self.client_id, self.client_secret
        );
        let resp = http::post(
            "https://apiauth.dk.team.blue/auth/realms/Curanet/protocol/openid-connect/token",
            data.as_bytes(),
            "application/x-www-form-urlencoded",
            &[],
        ).map_err(|e| Error::Provider(format!("Curanet auth: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Curanet auth: {e}")))?;
        let token = j::get_string_required(&v, &["access_token"])?;
        Ok(token.to_string())
    }

    fn get_root(&self, domain: &str, token: &str) -> Result<String, Error> {
        let auth = format!("Bearer {token}");
        let headers: &[(&str, &str)] = &[("Authorization", &auth), ("Accept", "application/json")];
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                continue;
            }
            let url = format!("https://api.curanet.dk/dns/v1/Domains/{h}/Records");
            let resp = http::get(&url, headers)
                .map_err(|e| Error::Provider(format!("Curanet zone lookup: {e}")))?;
            if !resp.body.contains("Entity not found") && !resp.body.contains("Bad Request") {
                return Ok(h);
            }
        }
        Err(Error::Provider(format!("Curanet: zone not found for {domain}")))
    }
}
