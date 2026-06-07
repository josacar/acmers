use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::json as j;
use crate::providers::{DnsProvider, ProviderResult};

const BASE: &str = "https://api.dynect.net/REST";

pub struct Dyn {
    customer: String,
    username: String,
    password: String,
}

impl DnsProvider for Dyn {
    fn slug() -> &'static str {
        "dyn"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DYN_Customer", "DYN_Username", "DYN_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Dyn {
            customer: env.get("DYN_Customer")
                .ok_or_else(|| Error::Config("DYN_Customer required".into()))?
                .clone(),
            username: env.get("DYN_Username")
                .ok_or_else(|| Error::Config("DYN_Username required".into()))?
                .clone(),
            password: env.get("DYN_Password")
                .ok_or_else(|| Error::Config("DYN_Password required".into()))?
                .clone(),
        }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = self.create_session()?;
        let headers: &[(&str, &str)] = &[("Auth-Token", &token), ("Content-Type", "application/json")];
        let zone = self.find_zone(domain, &token)?;

        let url = format!("{BASE}/TXTRecord/{zone}/{name}/");
        let body = serde_json::json!({
            "rdata": {"txtdata": value},
            "ttl": "60",
        });
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("Dyn create TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Dyn create TXT: HTTP {} {}", resp.status, resp.body)));
        }

        let pub_url = format!("{BASE}/Zone/{zone}/");
        let pub_body = serde_json::json!({"publish": true});
        http::put(&pub_url, &serde_json::to_vec(&pub_body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("Dyn publish zone: {e}")))?;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let token = match self.create_session() {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };
        let headers: &[(&str, &str)] = &[("Auth-Token", &token), ("Content-Type", "application/json")];
        let zone = match self.find_zone(domain, &token) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };

        let url = format!("{BASE}/TXTRecord/{zone}/{name}/");
        let _ = http::CLIENT.delete(&url, headers);

        let pub_url = format!("{BASE}/Zone/{zone}/");
        let pub_body = serde_json::json!({"publish": true});
        let _ = http::put(&pub_url, &serde_json::to_vec(&pub_body).unwrap(), "application/json", headers);
        Ok(())
    }
}

impl Dyn {
    fn create_session(&self) -> Result<String, Error> {
        let body = serde_json::json!({
            "customer_name": self.customer,
            "user_name": self.username,
            "password": self.password,
        });
        let url = format!("{BASE}/Session/");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("Dyn session: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Dyn session response: {e}")))?;
        j::get_string_required(&v, &["data", "token"]).map(|s| s.to_string())
    }

    fn find_zone(&self, domain: &str, token: &str) -> Result<String, Error> {
        let headers: &[(&str, &str)] = &[("Auth-Token", token)];
        let url = format!("{BASE}/Zone/");
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("Dyn list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Dyn zones: {e}")))?;
        if let Some(zones) = v.get("data").and_then(|d| d.as_array()) {
            for z in zones {
                if let Some(name) = z.get("zone").and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        return Ok(name.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
