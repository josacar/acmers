use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Cn {
    username: String,
    password: String,
}

impl DnsProvider for Cn {
    fn slug() -> &'static str {
        "cn"
    }

    fn env_vars() -> &'static [&'static str] {
        &["CN_Username", "CN_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("CN_Username")
            .ok_or_else(|| Error::Config("CN_Username required".into()))?.clone();
        let password = env.get("CN_Password")
            .ok_or_else(|| Error::Config("CN_Password required".into()))?.clone();
        Ok(Box::new(Cn { username, password }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = self.login()?;
        let zone = self.resolve_zone(domain, &token)?;
        let url = format!("https://beta.api.core-networks.de/dnszones/{zone}/records");
        let body = serde_json::json!({
            "name": name,
            "type": "TXT",
            "data": value,
            "ttl": 120,
        });
        let auth = format!("Bearer {}", token);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("cn add TXT: {e}")))?;
        if resp.status >= 300 {
            return Err(Error::Provider(format!("cn add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = match self.login() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("warning: cn cleanup login failed: {e}");
                return Ok(());
            }
        };
        let zone = match self.resolve_zone(domain, &token) {
            Ok(z) => z,
            Err(e) => {
                eprintln!("warning: cn cleanup zone not found: {e}");
                return Ok(());
            }
        };
        match self.find_record_in_zone(&zone, name, value, &token) {
            Ok(Some(record_id)) => {
                let url = format!("https://beta.api.core-networks.de/dnszones/{zone}/records/{record_id}");
                let auth = format!("Bearer {}", token);
                http::delete(&url, &[("Authorization", &auth)]).ok();
                Ok(())
            }
            Ok(None) => Ok(()),
            Err(e) => {
                eprintln!("warning: cn cleanup failed: {e}");
                Ok(())
            }
        }
    }
}

impl Cn {
    fn login(&self) -> Result<String, Error> {
        let body = serde_json::json!({
            "login": self.username,
            "password": self.password,
        });
        let resp = http::post("https://beta.api.core-networks.de/auth/token", &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("cn login: {e}")))?;
        if resp.status >= 300 {
            return Err(Error::Provider(format!("cn login: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("cn login: {e}")))?;
        v.get("token").and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| Error::Provider(format!("cn login: no token in response: {}", resp.body)))
    }

    fn resolve_zone(&self, domain: &str, token: &str) -> Result<String, Error> {
        let auth = format!("Bearer {}", token);
        let resp = http::get("https://beta.api.core-networks.de/dnszones", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("cn list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("cn zones: {e}")))?;
        if let Some(zones) = v.as_array() {
            for z in zones {
                if let Some(zname) = z.get("name").and_then(|n| n.as_str()) {
                    if domain == zname || domain.ends_with(&format!(".{zname}")) {
                        return Ok(zname.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }

    fn find_record_in_zone(&self, zone: &str, name: &str, value: &str, token: &str) -> Result<Option<String>, Error> {
        let url = format!("https://beta.api.core-networks.de/dnszones/{zone}/records");
        let auth = format!("Bearer {}", token);
        let resp = http::get(&url, &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("cn list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("cn records: {e}")))?;
        if let Some(records) = v.as_array() {
            for record in records {
                let rtype = record.get("type").and_then(|t| t.as_str()).unwrap_or("");
                let rname = record.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let rdata = record.get("data").and_then(|d| d.as_str()).unwrap_or("");
                if rtype == "TXT" && rname == name && rdata == value {
                    if let Some(id) = record.get("id") {
                        let id_str = if id.is_number() {
                            id.as_i64().map(|n| n.to_string())
                        } else {
                            id.as_str().map(|s| s.to_string())
                        };
                        if let Some(id_str) = id_str {
                            return Ok(Some(id_str));
                        }
                    }
                }
            }
        }
        Ok(None)
    }
}
