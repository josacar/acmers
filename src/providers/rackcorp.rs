use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const API_ENDPOINT: &str = "https://api.rackcorp.net/api/rest/v2.4/json.php";

pub struct Rackcorp {
    api_uuid: String,
    api_secret: String,
}

impl DnsProvider for Rackcorp {
    fn slug() -> &'static str {
        "rackcorp"
    }

    fn env_vars() -> &'static [&'static str] {
        &["RACKCORP_APIUUID", "RACKCORP_APISECRET"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Rackcorp {
            api_uuid: env.get("RACKCORP_APIUUID")
                .ok_or_else(|| Error::Config("RACKCORP_APIUUID required".into()))?
                .clone(),
            api_secret: env.get("RACKCORP_APISECRET")
                .ok_or_else(|| Error::Config("RACKCORP_APISECRET required".into()))?
                .clone(),
        }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone, lookup) = self.resolve_zone(domain, name)?;
        let body = serde_json::json!({
            "APIUUID": self.api_uuid,
            "APISECRET": self.api_secret,
            "cmd": "dns.record.create",
            "name": zone,
            "type": "TXT",
            "lookup": lookup,
            "data": value,
            "ttl": 300,
        });
        let resp = http::post(API_ENDPOINT, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Accept", "application/json")])
            .map_err(|e| Error::Provider(format!("Rackcorp add TXT: {e}")))?;
        Self::check_ok(&resp.body, "Rackcorp add TXT")
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone, lookup) = match self.resolve_zone(domain, name) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let body = serde_json::json!({
            "APIUUID": self.api_uuid,
            "APISECRET": self.api_secret,
            "cmd": "dns.record.delete",
            "name": zone,
            "type": "TXT",
            "lookup": lookup,
            "data": value,
        });
        match http::post(API_ENDPOINT, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Accept", "application/json")]) {
            Ok(resp) => {
                if let Err(e) = Self::check_ok(&resp.body, "Rackcorp remove TXT") {
                    eprintln!("warning: cleanup failed: {e}");
                }
                Ok(())
            }
            Err(e) => {
                eprintln!("warning: cleanup failed: {e}");
                Ok(())
            }
        }
    }
}

impl Rackcorp {
    fn api_call(&self, cmd: &str, extra: &Value) -> Result<Value, Error> {
        let mut body = serde_json::json!({
            "APIUUID": self.api_uuid,
            "APISECRET": self.api_secret,
            "cmd": cmd,
        });
        if let Value::Object(map) = extra {
            for (k, v) in map {
                body[k] = v.clone();
            }
        }
        let resp = http::post(API_ENDPOINT, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Accept", "application/json")])
            .map_err(|e| Error::Provider(format!("Rackcorp API: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Provider(format!("Rackcorp API JSON parse: {e}")))?;
        if v.get("code").and_then(|c| c.as_str()) == Some("OK") {
            Ok(v)
        } else {
            Err(Error::Provider(format!("Rackcorp API error: {}", resp.body)))
        }
    }

    fn check_ok(body: &str, ctx: &str) -> ProviderResult {
        let v: Value = serde_json::from_str(body)
            .map_err(|e| Error::Provider(format!("{ctx}: JSON parse: {e}")))?;
        if v.get("code").and_then(|c| c.as_str()) == Some("OK") {
            Ok(())
        } else {
            Err(Error::Provider(format!("{ctx}: {}", body)))
        }
    }

    fn resolve_zone(&self, domain: &str, name: &str) -> Result<(String, String), Error> {
        let full = if name.is_empty() || name == "@" {
            domain.to_string()
        } else if name.ends_with(domain) {
            name.to_string()
        } else {
            format!("{name}.{domain}")
        };

        let parts: Vec<&str> = full.split('.').collect();
        for i in 0..parts.len() {
            let candidate = parts[i..].join(".");
            let resp = match self.api_call("dns.domain.getall", &serde_json::json!({"exactName": candidate})) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let matches = resp.get("matches").and_then(|m| m.as_i64()).unwrap_or(0);
            let resp_name = resp.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if matches == 1 && resp_name == candidate {
                let lookup = if i == 0 {
                    String::new()
                } else {
                    parts[..i].join(".")
                };
                return Ok((candidate, lookup));
            }
        }
        Err(Error::Provider(format!("Rackcorp: could not find zone for {full}")))
    }
}
