use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Hetznercloud {
    token: String,
}

impl DnsProvider for Hetznercloud {
    fn slug() -> &'static str {
        "hetznercloud"
    }

    fn env_vars() -> &'static [&'static str] {
        &["HETZNER_TOKEN"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("HETZNER_TOKEN")
            .ok_or_else(|| Error::Config("HETZNER_TOKEN required".into()))?
            .clone();
        Ok(Box::new(Hetznercloud { token }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let headers: &[(&str, &str)] = &[("Authorization", &auth), ("Accept", "application/json")];

        let (zone_id, rr_name) = self.resolve_zone(name, headers)?;
        let quoted_value = format!("\"{}\"", value);

        let rrset_url = format!("https://api.hetzner.cloud/v1/zones/{zone_id}/rrsets/{rr_name}/TXT");
        match http::get(&rrset_url, headers) {
            Ok(resp) if resp.status == 200 => {
                if let Ok(v) = serde_json::from_str::<Value>(&resp.body) {
                    if let Some(records) = v.get("records").and_then(|r| r.as_array()) {
                        for record in records {
                            if record.get("value").and_then(|v| v.as_str()) == Some(&quoted_value) {
                                return Ok(());
                            }
                        }
                    }
                }
            }
            Ok(resp) if resp.status == 404 => {}
            Ok(resp) => {
                return Err(Error::Provider(format!("Hetzner get rrset: HTTP {}", resp.status)));
            }
            Err(e) => {
                return Err(Error::Provider(format!("Hetzner get rrset: {e}")));
            }
        }

        let add_url = format!("https://api.hetzner.cloud/v1/zones/{zone_id}/rrsets/{rr_name}/TXT/actions/add_records");
        let body = serde_json::to_vec(&serde_json::json!({
            "ttl": 120,
            "records": [{"value": quoted_value}]
        })).unwrap();

        let resp = http::post(&add_url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("Hetzner add TXT: {e}")))?;

        if resp.status >= 400 {
            let msg = extract_error_message(&resp.body);
            return Err(Error::Provider(format!("Hetzner add TXT: HTTP {}: {}", resp.status, msg)));
        }

        self.handle_action_response(&resp.body, headers)
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let headers: &[(&str, &str)] = &[("Authorization", &auth), ("Accept", "application/json")];

        let (zone_id, rr_name) = match self.resolve_zone(name, headers) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let rrset_url = format!("https://api.hetzner.cloud/v1/zones/{zone_id}/rrsets/{rr_name}/TXT");
        let resp = match http::get(&rrset_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };

        if resp.status != 200 {
            return Ok(());
        }

        let quoted_value = format!("\"{}\"", value);
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let mut found = false;
        if let Some(records) = v.get("records").and_then(|r| r.as_array()) {
            for record in records {
                if record.get("value").and_then(|v| v.as_str()) == Some(&quoted_value) {
                    found = true;
                    break;
                }
            }
        }

        if !found {
            return Ok(());
        }

        let remove_url = format!("https://api.hetzner.cloud/v1/zones/{zone_id}/rrsets/{rr_name}/TXT/actions/remove_records");
        let body = serde_json::to_vec(&serde_json::json!({
            "records": [{"value": quoted_value}]
        })).unwrap();

        let resp = match http::post(&remove_url, &body, "application/json", headers) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("warning: cleanup failed: {e}");
                return Ok(());
            }
        };

        if resp.status >= 400 {
            eprintln!("warning: cleanup HTTP {}: {}", resp.status, resp.body);
            return Ok(());
        }

        let _ = self.handle_action_response(&resp.body, headers);
        Ok(())
    }
}

impl Hetznercloud {
    fn resolve_zone(&self, domain: &str, headers: &[(&str, &str)]) -> Result<(String, String), Error> {
        let domain_lower = domain.to_lowercase();
        let parts: Vec<&str> = domain_lower.split('.').collect();

        for i in 1..parts.len() {
            let candidate = parts[i..].join(".");

            if let Ok((zone_id, zone_name)) = self.lookup_zone(&candidate, headers) {
                let rr_name = if domain_lower == zone_name {
                    "@".to_string()
                } else {
                    let suffix = format!(".{}", zone_name);
                    domain_lower.strip_suffix(&suffix).unwrap_or(domain).to_string()
                };
                return Ok((zone_id, rr_name));
            }
        }

        Err(Error::Provider(format!("zone not found for {domain}")))
    }

    fn lookup_zone(&self, candidate: &str, headers: &[(&str, &str)]) -> Result<(String, String), Error> {
        let url = format!("https://api.hetzner.cloud/v1/zones/{candidate}");
        if let Ok(resp) = http::get(&url, headers) {
            if resp.status == 200 {
                if let Some((id, name)) = self.parse_zone_response(&resp.body, candidate)? {
                    return Ok((id, name));
                }
            }
        }

        let url = format!("https://api.hetzner.cloud/v1/zones?name={candidate}");
        if let Ok(resp) = http::get(&url, headers) {
            if resp.status == 200 {
                if let Ok(v) = serde_json::from_str::<Value>(&resp.body) {
                    if let Some(zones) = v.get("zones").and_then(|z| z.as_array()) {
                        for zone in zones {
                            if let (Some(id), Some(zone_name)) = (
                                Self::extract_id(zone),
                                zone.get("name").and_then(|n| n.as_str()),
                            ) {
                                let trimmed = zone_name.trim_end_matches('.').to_lowercase();
                                if trimmed == candidate {
                                    return Ok((id, trimmed));
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(Error::Provider(format!("zone not found: {candidate}")))
    }

    fn parse_zone_response(&self, body: &str, candidate: &str) -> Result<Option<(String, String)>, Error> {
        let v: Value = serde_json::from_str(body)
            .map_err(|e| Error::Json(format!("Hetzner zone response: {e}")))?;
        let zone = v.get("zone").unwrap_or(&v);
        if let (Some(id), Some(zone_name)) = (
            Self::extract_id(zone),
            zone.get("name").and_then(|n| n.as_str()),
        ) {
            let trimmed = zone_name.trim_end_matches('.').to_lowercase();
            if trimmed == candidate {
                return Ok(Some((id, trimmed)));
            }
        }
        Ok(None)
    }

    fn extract_id(zone: &Value) -> Option<String> {
        if let Some(id) = zone.get("id").and_then(|i| i.as_u64()) {
            Some(id.to_string())
        } else {
            zone.get("id").and_then(|i| i.as_str()).map(|s| s.to_string())
        }
    }
}

fn extract_error_message(body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<Value>(body) {
        if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
            return msg.to_string();
        }
        if let Some(error) = v.get("error") {
            if let Some(msg) = error.get("message").and_then(|m| m.as_str()) {
                return msg.to_string();
            }
        }
    }
    body.to_string()
}

impl Hetznercloud {
    fn handle_action_response(&self, body: &str, headers: &[(&str, &str)]) -> ProviderResult {
        let v: Value = match serde_json::from_str(body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        if let Some(failed) = v.get("failed_actions").and_then(|f| f.as_array()) {
            if !failed.is_empty() {
                let msg = failed[0].get("message").and_then(|m| m.as_str()).unwrap_or("unknown");
                return Err(Error::Provider(format!("Hetzner action failed: {msg}")));
            }
        }

        if let Some(actions) = v.get("actions").and_then(|a| a.as_array()) {
            for action in actions {
                if let Some(action_id) = action.get("id").and_then(|i| i.as_u64()) {
                    self.wait_for_action(action_id, headers)?;
                }
            }
        }

        Ok(())
    }

    fn wait_for_action(&self, action_id: u64, headers: &[(&str, &str)]) -> ProviderResult {
        for _ in 0..120 {
            let url = format!("https://api.hetzner.cloud/v1/actions/{action_id}");
            let resp = http::get(&url, headers)
                .map_err(|e| Error::Provider(format!("Hetzner action poll: {e}")))?;

            if resp.status != 200 {
                return Err(Error::Provider(format!("Hetzner action poll: HTTP {}", resp.status)));
            }

            let v: Value = serde_json::from_str(&resp.body)
                .map_err(|e| Error::Json(format!("Hetzner action response: {e}")))?;

            let action = v.get("action").unwrap_or(&v);
            if let Some(status) = action.get("status").and_then(|s| s.as_str()) {
                match status {
                    "success" => return Ok(()),
                    "error" => {
                        let msg = action.get("error")
                            .and_then(|e| e.get("message"))
                            .and_then(|m| m.as_str())
                            .unwrap_or("unknown");
                        return Err(Error::Provider(format!("Hetzner action error: {msg}")));
                    }
                    _ => {}
                }
            }

            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        Err(Error::Provider("Hetzner action timed out".into()))
    }
}
