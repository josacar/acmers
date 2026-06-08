use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct BestHosting {
    api_user: String,
    api_key: String,
}

impl DnsProvider for BestHosting {
    fn slug() -> &'static str {
        "bh"
    }

    fn env_vars() -> &'static [&'static str] {
        &["BH_API_USER", "BH_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_user = env.get("BH_API_USER")
            .ok_or_else(|| Error::Config("BH_API_USER required".into()))?.clone();
        let api_key = env.get("BH_API_KEY")
            .ok_or_else(|| Error::Config("BH_API_KEY required".into()))?.clone();
        Ok(Box::new(BestHosting { api_user, api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let fulldomain = format!("{name}.{domain}");
        let url = "https://best-hosting.cz/api/v1/dns";
        let body = serde_json::json!({"fulldomain": fulldomain, "txtvalue": value});
        let auth = self.auth_header();
        let resp = http::post(url, &serde_json::to_vec(&body).unwrap(), "application/json", &[
            ("Authorization", &auth),
            ("Accept", "application/json"),
        ]).map_err(|e| Error::Provider(format!("bh add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("bh add TXT: HTTP {}: {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("bh add TXT response: {e}")))?;
        let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("");
        if status != "success" {
            return Err(Error::Provider(format!("bh add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let fulldomain = format!("{name}.{domain}");
        match self.find_record(&fulldomain, value) {
            Ok(Some(id)) => self.delete_record(&id),
            Ok(None) => Ok(()),
            Err(e) => {
                eprintln!("warning: bh cleanup failed: {e}");
                Ok(())
            }
        }
    }
}

impl BestHosting {
    fn auth_header(&self) -> String {
        let creds = base64::encode_std(format!("{}:{}", self.api_user, self.api_key).as_bytes());
        format!("Basic {creds}")
    }

    fn find_record(&self, fulldomain: &str, value: &str) -> Result<Option<String>, Error> {
        let url = "https://best-hosting.cz/api/v1/dns";
        let auth = self.auth_header();
        let resp = http::get(url, &[
            ("Authorization", &auth),
            ("Accept", "application/json"),
        ]).map_err(|e| Error::Provider(format!("bh list DNS: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("bh list DNS: HTTP {}: {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("bh list DNS response: {e}")))?;
        if let Some(records) = v.as_array() {
            for record in records {
                let rname = record.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let rcontent = record.get("content").and_then(|c| c.as_str()).unwrap_or("");
                let rtype = record.get("type").and_then(|t| t.as_str()).unwrap_or("");
                if rname == fulldomain && rcontent == value && rtype == "TXT" {
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

    fn delete_record(&self, id: &str) -> Result<(), Error> {
        let url = format!("https://best-hosting.cz/api/v1/dns/{id}");
        let auth = self.auth_header();
        let resp = http::delete(&url, &[
            ("Authorization", &auth),
            ("Accept", "application/json"),
        ]).map_err(|e| Error::Provider(format!("bh delete DNS: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("bh delete DNS: HTTP {}: {}", resp.status, resp.body)));
        }
        Ok(())
    }
}
