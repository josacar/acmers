use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::json as j;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Cyon {
    username: String,
    password: String,
}

impl DnsProvider for Cyon {
    fn slug() -> &'static str {
        "cyon"
    }

    fn env_vars() -> &'static [&'static str] {
        &["CY_Username", "CY_Password", "CY_OTP_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Cyon {
            username: env.get("CY_Username")
                .ok_or_else(|| Error::Config("CY_Username required".into()))?
                .clone(),
            password: env.get("CY_Password")
                .ok_or_else(|| Error::Config("CY_Password required".into()))?
                .clone(),
        }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = self.get_token()?;
        let headers: &[(&str, &str)] = &[("Authorization", &format!("Bearer {token}"))];
        let zone_id = self.find_zone(domain, &token)?;

        let record_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let url = format!("https://my.cyon.ch/api/v1/dns/zones/{zone_id}/records");
        let body = serde_json::json!({
            "type": "TXT",
            "name": record_name,
            "value": value,
            "ttl": 120,
        });
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("cyon add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("cyon add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = match self.get_token() {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };
        let headers: &[(&str, &str)] = &[("Authorization", &format!("Bearer {token}"))];
        let zone_id = match self.find_zone(domain, &token) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };

        let list_url = format!("https://my.cyon.ch/api/v1/dns/zones/{zone_id}/records");
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let records: Option<&Vec<Value>> = if let Some(arr) = v.as_array() {
            Some(arr)
        } else {
            v.get("data").and_then(|d| d.as_array())
        };

        if let Some(arr) = records {
            for record in arr {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("value").and_then(|v| v.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() {
                            Some(n.to_string())
                        } else {
                            i.as_str().map(|s| s.to_string())
                        }
                    }) {
                        let del_url = format!("https://my.cyon.ch/api/v1/dns/zones/{zone_id}/records/{id}");
                        let _ = http::CLIENT.delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Cyon {
    fn get_token(&self) -> Result<String, Error> {
        let body = serde_json::json!({
            "username": self.username,
            "password": self.password,
        });
        let resp = http::post(
            "https://my.cyon.ch/api/v1/auth/login",
            &serde_json::to_vec(&body).unwrap(),
            "application/json",
            &[],
        )
        .map_err(|e| Error::Provider(format!("cyon login: {e}")))?;

        if resp.status >= 400 {
            return Err(Error::Provider(format!("cyon login: HTTP {} {}", resp.status, resp.body)));
        }

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("cyon login response: {e}")))?;

        if let Some(token) = j::get_string(&v, &["token"]) {
            return Ok(token.to_string());
        }
        if let Some(token) = j::get_string(&v, &["data", "token"]) {
            return Ok(token.to_string());
        }
        Err(Error::Provider("cyon: no token in login response".into()))
    }

    fn find_zone(&self, domain: &str, token: &str) -> Result<String, Error> {
        let headers: &[(&str, &str)] = &[("Authorization", &format!("Bearer {token}"))];
        let resp = http::get("https://my.cyon.ch/api/v1/dns/zones", headers)
            .map_err(|e| Error::Provider(format!("cyon list zones: {e}")))?;

        if resp.status >= 400 {
            return Err(Error::Provider(format!("cyon list zones: HTTP {}", resp.status)));
        }

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("cyon zones: {e}")))?;

        let zones: Option<&Vec<Value>> = if let Some(arr) = v.as_array() {
            Some(arr)
        } else {
            v.get("data").and_then(|d| d.as_array())
        };

        if let Some(arr) = zones {
            for z in arr {
                if let Some(name) = z.get("name").and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        if let Some(id) = z.get("id").and_then(|i| {
                            if let Some(n) = i.as_i64() {
                                Some(n.to_string())
                            } else {
                                i.as_str().map(|s| s.to_string())
                            }
                        }) {
                            return Ok(id);
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("cyon: zone not found for {domain}")))
    }
}
