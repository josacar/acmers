use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const MB_API: &str = "https://api.mythic-beasts.com/dns/v2/zones";
const MB_AUTH: &str = "https://auth.mythic-beasts.com/login";

pub struct MythicBeasts {
    api_key: String,
    api_secret: String,
}

impl DnsProvider for MythicBeasts {
    fn slug() -> &'static str {
        "mythic_beasts"
    }

    fn env_vars() -> &'static [&'static str] {
        &["MB_AK", "MB_AS"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("MB_AK")
            .ok_or_else(|| Error::Config("MB_AK required".into()))?
            .clone();
        let api_secret = env.get("MB_AS")
            .ok_or_else(|| Error::Config("MB_AS required".into()))?
            .clone();
        Ok(Box::new(MythicBeasts { api_key, api_secret }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = self.oauth2_token()?;
        let bearer = format!("Bearer {token}");
        let headers: &[(&str, &str)] = &[
            ("Authorization", &bearer),
            ("Accepts", "application/json"),
        ];
        let (zone, sub_domain) = self.resolve_zone(name, headers)?;

        let url = format!("{MB_API}/{zone}/records/{sub_domain}/TXT");
        let body = format!("data={value}");
        let resp = http::post(&url, body.as_bytes(), "application/x-www-form-urlencoded", headers)
            .map_err(|e| Error::Provider(format!("MythicBeasts add TXT: {e}")))?;
        if resp.status >= 400 || resp.body.contains("\"error\"") || resp.body.contains("invalid_client") {
            return Err(Error::Provider(format!("MythicBeasts add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        if !resp.body.contains("1 records added") {
            return Err(Error::Provider(format!("MythicBeasts add TXT: unexpected response: {}", resp.body)));
        }

        for _ in 0..6 {
            let verify_url = format!("{MB_API}/{zone}/records/{sub_domain}/TXT?verify");
            match http::get(&verify_url, headers) {
                Ok(r) if r.status < 400 && !r.body.contains("\"error\"") => {
                    return Ok(());
                }
                _ => {}
            }
            thread::sleep(Duration::from_secs(20));
        }
        Err(Error::Provider("MythicBeasts: record not verified after retries".into()))
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = match self.oauth2_token() {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };
        let bearer = format!("Bearer {token}");
        let headers: &[(&str, &str)] = &[
            ("Authorization", &bearer),
            ("Accepts", "application/json"),
        ];
        let (zone, sub_domain) = match self.resolve_zone(name, headers) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let url = format!("{MB_API}/{zone}/records/{sub_domain}/TXT");
        let body = format!("data={value}");
        let _ = http::delete_with_body(&url, body.as_bytes(), "application/x-www-form-urlencoded", headers);
        Ok(())
    }
}

impl MythicBeasts {
    fn oauth2_token(&self) -> Result<String, Error> {
        let creds = format!("{}:{}", self.api_key, self.api_secret);
        let basic = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        let headers: &[(&str, &str)] = &[
            ("Authorization", &basic),
            ("Accepts", "application/json"),
        ];
        let body = b"grant_type=client_credentials";
        let resp = http::post(MB_AUTH, body, "application/x-www-form-urlencoded", headers)
            .map_err(|e| Error::Provider(format!("MythicBeasts OAuth2: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("MythicBeasts OAuth2: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("MythicBeasts OAuth2: {e}")))?;
        if v.get("token_type").and_then(|t| t.as_str()) != Some("bearer") {
            return Err(Error::Provider(format!("MythicBeasts OAuth2: token_type not bearer: {}", resp.body)));
        }
        let token = v.get("access_token").and_then(|t| t.as_str())
            .ok_or_else(|| Error::Provider(format!("MythicBeasts OAuth2: no access_token: {}", resp.body)))?;
        Ok(token.to_string())
    }

    fn resolve_zone(&self, fulldomain: &str, headers: &[(&str, &str)]) -> Result<(String, String), Error> {
        let parts: Vec<&str> = fulldomain.split('.').collect();
        let mut p = 0usize;
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            let url = format!("{MB_API}/{h}/records");
            match http::get(&url, headers) {
                Ok(resp) if resp.status < 400 && !resp.body.contains("\"error\"") => {
                    let sub_domain = if p == 0 {
                        String::new()
                    } else {
                        parts[..p].join(".")
                    };
                    return Ok((h, sub_domain));
                }
                Ok(resp) if resp.body.contains("\"error\"") && resp.status == 403 => {
                    return Err(Error::Provider(format!("MythicBeasts access denied for {h}")));
                }
                _ => {}
            }
            p = i + 1;
        }
        Err(Error::Provider(format!("zone not found for {fulldomain}")))
    }
}
