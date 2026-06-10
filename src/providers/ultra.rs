use std::collections::HashMap;

use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::json as j;
use crate::providers::{DnsProvider, ProviderResult};

const API_URL: &str = "https://api.ultradns.com/v3/";
const AUTH_URL: &str = "https://api.ultradns.com/v2/authorization/token";

pub struct Ultra {
    username: String,
    password: String,
}

impl DnsProvider for Ultra {
    fn slug() -> &'static str {
        "ultra"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ULTRA_USR", "ULTRA_PWD"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("ULTRA_USR")
            .ok_or_else(|| Error::Config("ULTRA_USR required".into()))?
            .clone();
        let password = env.get("ULTRA_PWD")
            .ok_or_else(|| Error::Config("ULTRA_PWD required".into()))?
            .clone();
        Ok(Box::new(Ultra { username, password }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = self.obtain_token()?;
        let (zone_id, sub_domain) = self.get_root(name, &token)?;
        let bearer = format!("Bearer {token}");
        let headers: &[(&str, &str)] = &[
            ("Content-Type", "application/json"),
            ("Authorization", &bearer),
        ];

        let check_url = format!("{API_URL}zones/{zone_id}/rrsets/TXT?q=value:{name}");
        let resp = http::get(&check_url, headers)
            .map_err(|e| Error::Provider(format!("ultra check record: {e}")))?;
        if resp.body.contains("\"totalCount\"") {
            return Err(Error::Provider("ultra: TXT record already exists".into()));
        }

        let url = format!("{API_URL}zones/{zone_id}/rrsets/TXT/{sub_domain}");
        let body = serde_json::json!({
            "ttl": 300,
            "rdata": [value],
        });
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("ultra add TXT: {e}")))?;
        if resp.body.contains("Successful") || resp.body.contains("already exists") {
            return Ok(());
        }
        if resp.status >= 400 {
            return Err(Error::Provider(format!("ultra add TXT: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = match self.obtain_token() {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };
        let (zone_id, sub_domain) = match self.get_root(name, &token) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let bearer = format!("Bearer {token}");
        let headers: &[(&str, &str)] = &[
            ("Content-Type", "application/json"),
            ("Authorization", &bearer),
        ];

        let list_url = format!("{API_URL}zones/{zone_id}/rrsets?q=kind:RECORDS+owner:{sub_domain}");
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if !resp.body.contains("\"resultInfo\"") {
            return Ok(());
        }

        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let count = j::get_value(&v, &["resultInfo", "returnedCount"])
            .and_then(|c| c.as_u64())
            .unwrap_or(0);
        if count == 0 {
            return Ok(());
        }

        let del_url = format!("{API_URL}zones/{zone_id}/rrsets/TXT/{sub_domain}");
        let body = serde_json::json!({
            "ttl": 300,
            "rdata": [value],
        });
        let _ = http::delete_with_body(&del_url, &serde_json::to_vec(&body).unwrap(), "application/json", headers);
        Ok(())
    }
}

impl Ultra {
    fn obtain_token(&self) -> Result<String, Error> {
        let form = format!(
            "grant_type=password&username={}&password={}",
            self.username, self.password
        );
        let resp = http::post(AUTH_URL, form.as_bytes(), "application/x-www-form-urlencoded", &[])
            .map_err(|e| Error::Provider(format!("ultra auth: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("ultra auth: {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("ultra auth parse: {e}")))?;
        let token = j::get_string_required(&v, &["access_token"])
            .map_err(|_| Error::Provider(format!("ultra auth: no access_token in response: {}", resp.body)))?;
        Ok(token.to_string())
    }

    fn get_root(&self, fulldomain: &str, token: &str) -> Result<(String, String), Error> {
        let bearer = format!("Bearer {token}");
        let headers: &[(&str, &str)] = &[
            ("Content-Type", "application/json"),
            ("Authorization", &bearer),
        ];

        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 1..parts.len() {
            let candidate = parts[i..].join(".");
            if !candidate.contains('.') {
                break;
            }
            let resp = http::get(&format!("{API_URL}zones"), headers)
                .map_err(|e| Error::Provider(format!("ultra list zones: {e}")))?;
            if resp.body.contains(&format!("\"{candidate}.\"")) {
                let zone_id = format!("{candidate}.");
                let sub_domain = parts[..i].join(".");
                return Ok((zone_id, sub_domain));
            }
        }
        Err(Error::Provider(format!("ultra: zone not found for {fulldomain}")))
    }
}
