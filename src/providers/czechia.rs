use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const DEFAULT_API_BASE: &str = "https://api.czechia.com";

pub struct Czechia {
    auth_token: String,
    zones: Vec<String>,
    api_base: String,
}

impl DnsProvider for Czechia {
    fn slug() -> &'static str {
        "czechia"
    }

    fn env_vars() -> &'static [&'static str] {
        &["CZ_AuthorizationToken", "CZ_Zones", "CZ_API_BASE"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let auth_token = env.get("CZ_AuthorizationToken")
            .ok_or_else(|| Error::Config("CZ_AuthorizationToken required".into()))?
            .trim()
            .to_string();
        let zones_raw = env.get("CZ_Zones")
            .ok_or_else(|| Error::Config("CZ_Zones required".into()))?;
        let zones: Vec<String> = zones_raw
            .split(|c: char| c == ',' || c.is_whitespace())
            .map(|s| s.trim().trim_end_matches('.').to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        if zones.is_empty() {
            return Err(Error::Config("CZ_Zones must contain at least one zone".into()));
        }
        let api_base = env
            .get("CZ_API_BASE")
            .map(|s| s.trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_API_BASE.to_string());
        Ok(Box::new(Czechia { auth_token, zones, api_base }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = self.resolve_zone(domain)?;
        let host = Self::host_name(name, &zone);
        let body = serde_json::json!({
            "hostName": host,
            "text": value,
            "ttl": 300,
            "publishZone": 1,
        });
        let url = format!("{}/api/DNS/{}/TXT", self.api_base, zone);
        let resp = http::post(
            &url,
            &serde_json::to_vec(&body).unwrap(),
            "application/json",
            &[("AuthorizationToken", &self.auth_token)],
        )
        .map_err(|e| Error::Provider(format!("czechia add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!(
                "czechia add TXT: HTTP {} {}",
                resp.status, resp.body
            )));
        }
        if resp.body.contains("already exists") {
            return Ok(());
        }
        if Self::is_error_response(&resp.body) {
            return Err(Error::Provider(format!("czechia add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = match self.resolve_zone(domain) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let host = Self::host_name(name, &zone);
        let body = serde_json::json!({
            "hostName": host,
            "text": value,
            "ttl": 300,
            "publishZone": 1,
        });
        let url = format!("{}/api/DNS/{}/TXT", self.api_base, zone);
        let resp = match http::delete_with_body(
            &url,
            &serde_json::to_vec(&body).unwrap(),
            "application/json",
            &[("AuthorizationToken", &self.auth_token)],
        ) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("warning: czechia cleanup request failed: {e}");
                return Ok(());
            }
        };
        if resp.body.contains("\"isError\":true") || resp.body.contains("\"isError\": true") {
            eprintln!("warning: czechia cleanup error: {}", resp.body);
        }
        Ok(())
    }
}

impl Czechia {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let fd = domain.to_lowercase().trim_end_matches('.').to_string();
        let mut best: Option<&str> = None;
        for z in &self.zones {
            if fd == *z || fd.ends_with(&format!(".{z}")) {
                if best.is_none_or(|b| z.len() > b.len()) {
                    best = Some(z.as_str());
                }
            }
        }
        best.map(|s| s.to_string())
            .ok_or_else(|| Error::Provider(format!("czechia: no matching zone for {domain}")))
    }

    fn host_name(fqdn: &str, zone: &str) -> String {
        let fd = fqdn.to_lowercase().trim_end_matches('.').to_string();
        if fd == zone {
            return "@".to_string();
        }
        if let Some(stripped) = fd.strip_suffix(&format!(".{zone}")) {
            if stripped.is_empty() {
                return "@".to_string();
            }
            return stripped.to_string();
        }
        fd
    }

    fn is_error_response(body: &str) -> bool {
        body.contains("\"status\":4")
            || body.contains("\"status\":5")
            || body.contains("\"errors\"")
    }
}
