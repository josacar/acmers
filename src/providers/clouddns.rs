use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::json as j;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Clouddns {
    token: String,
}

impl DnsProvider for Clouddns {
    fn slug() -> &'static str {
        "clouddns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["CLOUDDNS_CLIENT_ID", "CLOUDDNS_EMAIL", "CLOUDDNS_PASSWORD"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let client_id = env.get("CLOUDDNS_CLIENT_ID")
            .ok_or_else(|| Error::Config("CLOUDDNS_CLIENT_ID required".into()))?.clone();
        let email = env.get("CLOUDDNS_EMAIL")
            .ok_or_else(|| Error::Config("CLOUDDNS_EMAIL required".into()))?.clone();
        let password = env.get("CLOUDDNS_PASSWORD")
            .ok_or_else(|| Error::Config("CLOUDDNS_PASSWORD required".into()))?.clone();

        let login_body = serde_json::json!({
            "clientId": client_id,
            "email": email,
            "password": password,
        });
        let resp = http::post(
            "https://api.clouddns.net/api/v1/login",
            &serde_json::to_vec(&login_body).unwrap(),
            "application/json",
            &[],
        ).map_err(|e| Error::Provider(format!("clouddns login: {e}")))?;

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("clouddns login: {e}")))?;

        if let Some(error) = v.get("error").and_then(|e| e.as_str()) {
            return Err(Error::Provider(format!("clouddns login: {error}")));
        }

        let token = j::get_string_required(&v, &["data", "token"])?.to_string();

        Ok(Box::new(Clouddns { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = self.resolve_domain(domain)?;
        let url = format!("https://api.clouddns.net/api/v1/domains/{domain_id}/records");
        let body = serde_json::json!({
            "type": "TXT",
            "name": name,
            "content": value,
            "ttl": 120,
        });
        let auth = format!("Bearer {}", self.token);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[
            ("Authorization", &auth),
        ]).map_err(|e| Error::Provider(format!("clouddns add TXT: {e}")))?;

        if resp.status >= 400 {
            return Err(Error::Provider(format!("clouddns add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = match self.resolve_domain(domain) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("warning: clouddns cleanup zone not found: {e}");
                return Ok(());
            }
        };
        let auth = format!("Bearer {}", self.token);

        let list_url = format!("https://api.clouddns.net/api/v1/domains/{domain_id}/records");
        let resp = match http::get(&list_url, &[("Authorization", &auth)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };

        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        if let Some(records) = v.get("data").and_then(|d| d.as_array()) {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| i.as_str()) {
                        let del_url = format!("https://api.clouddns.net/api/v1/domains/{domain_id}/records/{id}");
                        let _ = http::delete(&del_url, &[("Authorization", &auth)]);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Clouddns {
    fn resolve_domain(&self, domain: &str) -> Result<String, Error> {
        let auth = format!("Bearer {}", self.token);
        let resp = http::get("https://api.clouddns.net/api/v1/domains", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("clouddns list domains: {e}")))?;

        if resp.status >= 400 {
            return Err(Error::Provider(format!("clouddns list domains: HTTP {} {}", resp.status, resp.body)));
        }

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("clouddns domains: {e}")))?;

        if let Some(domains) = v.get("data").and_then(|d| d.as_array()) {
            for d in domains {
                if let Some(name) = d.get("name").and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        if let Some(id) = d.get("id") {
                            let id_str = if id.is_number() {
                                id.as_i64().map(|n| n.to_string())
                            } else {
                                id.as_str().map(|s| s.to_string())
                            };
                            if let Some(id_str) = id_str {
                                return Ok(id_str);
                            }
                        }
                    }
                }
            }
        }

        Err(Error::Provider(format!("domain not found for {domain}")))
    }
}
