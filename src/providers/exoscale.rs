use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Exoscale {
    api_key: String,
    api_secret: String,
}

impl DnsProvider for Exoscale {
    fn slug() -> &'static str {
        "exoscale"
    }

    fn env_vars() -> &'static [&'static str] {
        &["EXOSCALE_API_KEY", "EXOSCALE_API_SECRET"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("EXOSCALE_API_KEY")
            .ok_or_else(|| Error::Config("EXOSCALE_API_KEY required".into()))?
            .clone();
        let api_secret = env.get("EXOSCALE_API_SECRET")
            .ok_or_else(|| Error::Config("EXOSCALE_API_SECRET required".into()))?
            .clone();
        Ok(Box::new(Exoscale { api_key, api_secret }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let basic_auth = make_basic(&self.api_key, &self.api_secret);
        let url = format!("https://api.exoscale.com/dns/v1/domains/{zone_id}/records");
        let body = serde_json::json!({
            "record": {
                "name": name,
                "record_type": "TXT",
                "content": value,
                "ttl": 120,
            }
        });
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &basic_auth)])
            .map_err(|e| Error::Provider(format!("exoscale add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("exoscale add TXT: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let basic_auth = make_basic(&self.api_key, &self.api_secret);
        let list_url = format!("https://api.exoscale.com/dns/v1/domains/{zone_id}/records");
        let resp = http::get(&list_url, &[("Authorization", &basic_auth)])
            .map_err(|e| Error::Provider(format!("exoscale list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("exoscale parse: {e}")))?;
        if let Some(records) = v.as_array() {
            for rec in records {
                let rec_type = rec.get("record_type").and_then(|t| t.as_str());
                let rec_name = rec.get("name").and_then(|n| n.as_str());
                let rec_content = rec.get("content").and_then(|c| c.as_str());
                if rec_type == Some("TXT") && rec_name == Some(name) && rec_content == Some(value) {
                    if let Some(id) = rec.get("id").and_then(|i| i.as_str()) {
                        let del_url = format!("https://api.exoscale.com/dns/v1/domains/{zone_id}/records/{id}");
                        http::delete(&del_url, &[("Authorization", &basic_auth)]).ok();
                    }
                }
            }
        }
        Ok(())
    }
}

impl Exoscale {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let basic_auth = make_basic(&self.api_key, &self.api_secret);
        let url = "https://api.exoscale.com/dns/v1/domains";
        let resp = http::get(url, &[("Authorization", &basic_auth)])
            .map_err(|e| Error::Provider(format!("exoscale list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("exoscale parse: {e}")))?;
        if let Some(domains) = v.get("domains").and_then(|d| d.as_array()) {
            for d in domains {
                if let Some(name) = d.get("name").and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        if let Some(id) = d.get("id").and_then(|i| i.as_str()) {
                            return Ok(id.to_string());
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("exoscale zone not found for {domain}")))
    }
}

fn make_basic(username: &str, password: &str) -> String {
    let creds = format!("{username}:{password}");
    format!("Basic {}", base64::encode(creds.as_bytes()))
}
