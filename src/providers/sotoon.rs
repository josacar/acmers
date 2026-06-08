use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const API_URL: &str = "https://api.sotoon.ir/delivery/v2.1/global";

pub struct Sotoon {
    token: String,
    workspace_uuid: String,
}

impl DnsProvider for Sotoon {
    fn slug() -> &'static str { "sotoon" }
    fn env_vars() -> &'static [&'static str] { &["Sotoon_Token", "Sotoon_WorkspaceUUID"] }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("Sotoon_Token")
            .ok_or_else(|| Error::Config("Sotoon_Token required".into()))?.clone();
        let workspace_uuid = env.get("Sotoon_WorkspaceUUID")
            .ok_or_else(|| Error::Config("Sotoon_WorkspaceUUID required".into()))?.clone();
        Ok(Box::new(Sotoon { token, workspace_uuid }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let (zone_name, sub_domain) = self.resolve_zone(domain, name, &auth)?;

        let zone_url = format!("{API_URL}/workspaces/{}/domainzones/{}", self.workspace_uuid, zone_name);
        let resp = http::get(&zone_url, &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("sotoon get zone: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("sotoon get zone: HTTP {} {}", resp.status, resp.body)));
        }

        let zone: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Provider(format!("sotoon parse zone: {e}")))?;

        let existing = zone.get("spec").and_then(|s| s.get("records"))
            .and_then(|r| r.get(&sub_domain))
            .and_then(|v| v.as_array());

        let mut records = Vec::new();
        if let Some(arr) = existing {
            for item in arr {
                if let Some(txt_val) = item.get("TXT").and_then(|t| t.as_str()) {
                    if txt_val == value {
                        return Ok(());
                    }
                }
                records.push(item.clone());
            }
        }
        records.push(serde_json::json!({"TXT": value, "type": "TXT", "ttl": 120}));

        let body = serde_json::json!({
            "spec": {"records": {sub_domain: records}}
        });
        let resp = http::patch(&zone_url, &serde_json::to_vec(&body).unwrap(),
            "application/merge-patch+json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("sotoon add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("sotoon add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let (zone_name, sub_domain) = self.resolve_zone(domain, name, &auth)?;

        let zone_url = format!("{API_URL}/workspaces/{}/domainzones/{}", self.workspace_uuid, zone_name);
        let resp = match http::get(&zone_url, &[("Authorization", &auth)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if resp.status >= 400 {
            return Ok(());
        }

        let zone: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let existing = zone.get("spec").and_then(|s| s.get("records"))
            .and_then(|r| r.get(&sub_domain))
            .and_then(|v| v.as_array());

        let remaining: Vec<Value> = match existing {
            Some(arr) => arr.iter().filter(|item| {
                let txt_val = item.get("TXT").and_then(|t| t.as_str());
                txt_val != Some(value)
            }).cloned().collect(),
            None => return Ok(()),
        };

        let records_value = if remaining.is_empty() {
            Value::Null
        } else {
            Value::Array(remaining)
        };

        let body = serde_json::json!({
            "spec": {"records": {sub_domain: records_value}}
        });
        let _ = http::patch(&zone_url, &serde_json::to_vec(&body).unwrap(),
            "application/merge-patch+json", &[("Authorization", &auth)]);
        Ok(())
    }
}

impl Sotoon {
    fn resolve_zone(&self, domain: &str, name: &str, auth: &str) -> Result<(String, String), Error> {
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            let list_url = format!("{API_URL}/workspaces/{}/domainzones", self.workspace_uuid);
            let resp = http::get(&list_url, &[("Authorization", auth)])
                .map_err(|e| Error::Provider(format!("sotoon list zones: {e}")))?;
            if resp.status >= 400 {
                continue;
            }

            let zones: Value = match serde_json::from_str(&resp.body) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let items = zones.as_array()
                .or_else(|| zones.get("items").and_then(|i| i.as_array()));

            if let Some(items) = items {
                for item in items {
                    let origin = item.get("spec").and_then(|s| s.get("origin"))
                        .and_then(|o| o.as_str());
                    if origin == Some(&h) {
                        let zone_name = item.get("metadata").and_then(|m| m.get("name"))
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| h.replace('.', "-"));
                        let sub_domain = name.strip_suffix(&format!(".{h}"))
                            .unwrap_or(name)
                            .to_string();
                        return Ok((zone_name, sub_domain));
                    }
                }
            }
        }
        Err(Error::Provider("sotoon: could not find zone for domain".into()))
    }
}
