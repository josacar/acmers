use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.wedos.com/wapi/json";

pub struct Wedos {
    user: String,
    password: String,
}

impl DnsProvider for Wedos {
    fn slug() -> &'static str {
        "wedos"
    }

    fn env_vars() -> &'static [&'static str] {
        &["WEDOS_User", "WEDOS_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let user = env.get("WEDOS_User")
            .ok_or_else(|| Error::Config("WEDOS_User required".into()))?
            .clone();
        let password = env.get("WEDOS_Password")
            .ok_or_else(|| Error::Config("WEDOS_Password required".into()))?
            .clone();
        Ok(Box::new(Wedos { user, password }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_name = self.resolve_domain(domain)?;
        let rec_name = name.strip_suffix(&format!(".{domain_name}")).unwrap_or(name);

        let body = serde_json::json!({
            "request": "dns_record_add",
            "auth": {
                "user": self.user,
                "pass": self.password,
            },
            "domain": domain_name,
            "name": rec_name,
            "type": "TXT",
            "data": value,
            "ttl": 120,
        });
        let resp = http::post(BASE_URL, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("wedos add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("wedos add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("wedos response: {e}")))?;
        if v.get("code").and_then(|c| c.as_i64()) != Some(1000) {
            let msg = v.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error");
            return Err(Error::Provider(format!("wedos add TXT: {msg}")));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_name = match self.resolve_domain(domain) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };
        let rec_name = name.strip_suffix(&format!(".{domain_name}")).unwrap_or(name);

        let record_id = match self.find_record_id(&domain_name, rec_name, value) {
            Ok(Some(id)) => id,
            _ => return Ok(()),
        };

        let body = serde_json::json!({
            "request": "dns_record_delete",
            "auth": {
                "user": self.user,
                "pass": self.password,
            },
            "domain": domain_name,
            "id": record_id,
        });
        let resp = http::post(BASE_URL, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("wedos delete TXT: {e}")))?;
        if resp.status >= 400 {
            return Ok(());
        }
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if v.get("code").and_then(|c| c.as_i64()) != Some(1000) {
            return Ok(());
        }
        Ok(())
    }
}

impl Wedos {
    fn resolve_domain(&self, domain: &str) -> Result<String, Error> {
        let body = serde_json::json!({
            "request": "dns_domain_list",
            "auth": {
                "user": self.user,
                "pass": self.password,
            },
        });
        let resp = http::post(BASE_URL, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("wedos list domains: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("wedos list domains: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("wedos domain list: {e}")))?;

        if v.get("code").and_then(|c| c.as_i64()) != Some(1000) {
            let msg = v.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error");
            return Err(Error::Provider(format!("wedos auth: {msg}")));
        }

        if let Some(domains) = v.get("data").and_then(|d| d.get("domain")).and_then(|d| d.as_array()) {
            for d in domains {
                if let Some(name) = d.get("name").and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        return Ok(name.to_string());
                    }
                }
            }
        }

        if let Some(domains) = v.get("data").and_then(|d| d.as_array()) {
            for d in domains {
                if let Some(name) = d.get("name").and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        return Ok(name.to_string());
                    }
                }
            }
        }

        Err(Error::Provider(format!("wedos: domain not found for {domain}")))
    }

    fn find_record_id(&self, domain_name: &str, rec_name: &str, value: &str) -> Result<Option<i64>, Error> {
        let body = serde_json::json!({
            "request": "dns_record_list",
            "auth": {
                "user": self.user,
                "pass": self.password,
            },
            "domain": domain_name,
        });
        let resp = http::post(BASE_URL, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("wedos list records: {e}")))?;

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("wedos record list: {e}")))?;

        let records = v.get("data").and_then(|d| d.as_array())
            .or_else(|| v.get("data").and_then(|d| d.get("record")).and_then(|r| r.as_array()));

        if let Some(records) = records {
            for r in records {
                if r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && r.get("name").and_then(|n| n.as_str()) == Some(rec_name)
                    && r.get("data").and_then(|d| d.as_str()) == Some(value)
                {
                    if let Some(id) = r.get("id").and_then(|i| i.as_i64()) {
                        return Ok(Some(id));
                    }
                }
            }
        }
        Ok(None)
    }
}
