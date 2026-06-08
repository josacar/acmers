use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Pdnsmanager {
    api_key: String,
    api_password: String,
    api_url: String,
}

impl DnsProvider for Pdnsmanager {
    fn slug() -> &'static str {
        "pdnsmanager"
    }

    fn env_vars() -> &'static [&'static str] {
        &["PDNSMGR_API_KEY", "PDNSMGR_API_PASSWORD", "PDNSMGR_API_URL"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("PDNSMGR_API_KEY")
            .ok_or_else(|| Error::Config("PDNSMGR_API_KEY required".into()))?
            .clone();
        let api_password = env.get("PDNSMGR_API_PASSWORD")
            .ok_or_else(|| Error::Config("PDNSMGR_API_PASSWORD required".into()))?
            .clone();
        let api_url = env.get("PDNSMGR_API_URL")
            .ok_or_else(|| Error::Config("PDNSMGR_API_URL required".into()))?
            .trim_end_matches('/')
            .to_string();
        Ok(Box::new(Pdnsmanager { api_key, api_password, api_url }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let rec_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let creds = base64::encode_std(format!("{}:{}", self.api_key, self.api_password).as_bytes());
        let auth = format!("Basic {creds}");
        let body = serde_json::json!({
            "type": "TXT",
            "name": rec_name,
            "content": value,
            "ttl": 120,
        });
        let url = format!("{}/dns/zones/{domain}/records", self.api_url);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("pdnsmanager add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("pdnsmanager add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let rec_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let creds = base64::encode_std(format!("{}:{}", self.api_key, self.api_password).as_bytes());
        let auth = format!("Basic {creds}");
        let list_url = format!("{}/dns/zones/{domain}/records", self.api_url);
        let resp = match http::get(&list_url, &[("Authorization", &auth)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records = v.as_array()
            .or_else(|| v.get("data").and_then(|d| d.as_array()))
            .or_else(|| v.get("records").and_then(|r| r.as_array()));
        if let Some(records) = records {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(rec_name)
                {
                    if let Some(id) = record_id(record) {
                        let del_url = format!("{}/dns/zones/{domain}/records/{id}", self.api_url);
                        let _ = http::delete(&del_url, &[("Authorization", &auth)]);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

fn record_id(v: &Value) -> Option<String> {
    v.get("id").and_then(|i| {
        if i.is_string() { i.as_str().map(|s| s.to_string()) }
        else if i.is_u64() { i.as_u64().map(|n| n.to_string()) }
        else if i.is_i64() { i.as_i64().map(|n| n.to_string()) }
        else { None }
    })
}
