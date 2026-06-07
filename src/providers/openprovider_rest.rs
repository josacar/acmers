use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct OpenproviderRest {
    username: String,
    password: String,
}

impl DnsProvider for OpenproviderRest {
    fn slug() -> &'static str {
        "openprovider_rest"
    }

    fn env_vars() -> &'static [&'static str] {
        &["OPENPROVIDER_REST_Username", "OPENPROVIDER_REST_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("OPENPROVIDER_REST_Username")
            .ok_or_else(|| Error::Config("OPENPROVIDER_REST_Username required".into()))?
            .clone();
        let password = env.get("OPENPROVIDER_REST_Password")
            .ok_or_else(|| Error::Config("OPENPROVIDER_REST_Password required".into()))?
            .clone();
        Ok(Box::new(OpenproviderRest { username, password }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = self.get_token()?;
        let headers: &[(&str, &str)] = &[("Authorization", &token)];
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "TXT",
            "name": name,
            "value": value,
            "ttl": 120,
        })).unwrap();
        let url = format!("https://api.openprovider.eu/v1beta/dns/zones/{domain}/records");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("OpenProvider add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("OpenProvider add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let token = match self.get_token() {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };
        let headers: &[(&str, &str)] = &[("Authorization", &token)];
        let del_url = format!("https://api.openprovider.eu/v1beta/dns/zones/{domain}/records/{name}");
        let _ = http::delete(&del_url, headers);
        Ok(())
    }
}

impl OpenproviderRest {
    fn get_token(&self) -> Result<String, Error> {
        let body = serde_json::to_vec(&serde_json::json!({
            "username": self.username,
            "password": self.password,
        })).unwrap();
        let resp = http::post("https://api.openprovider.eu/v1beta/auth/login", &body, "application/json", &[])
            .map_err(|e| Error::Provider(format!("OpenProvider auth: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("OpenProvider auth: {e}")))?;
        let token = v.pointer("/data/token").and_then(|t| t.as_str())
            .ok_or_else(|| Error::Provider(format!("OpenProvider auth: no token in response: {}", resp.body)))?;
        Ok(format!("Bearer {token}"))
    }
}
