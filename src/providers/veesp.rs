use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.veesp.com/v1";

pub struct Veesp {
    username: String,
    password: String,
}

impl DnsProvider for Veesp {
    fn slug() -> &'static str {
        "veesp"
    }

    fn env_vars() -> &'static [&'static str] {
        &["VEESP_Username", "VEESP_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("VEESP_Username")
            .ok_or_else(|| Error::Config("VEESP_Username required".into()))?
            .clone();
        let password = env.get("VEESP_Password")
            .ok_or_else(|| Error::Config("VEESP_Password required".into()))?
            .clone();
        Ok(Box::new(Veesp { username, password }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = auth_header(&self.username, &self.password);
        let rec_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);

        let body = serde_json::json!({
            "type": "TXT",
            "name": rec_name,
            "content": value,
            "ttl": 120,
        });
        let url = format!("{BASE_URL}/domains/{domain}/records");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("veesp add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("veesp add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let auth = auth_header(&self.username, &self.password);
        let rec_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);

        let list_url = format!("{BASE_URL}/domains/{domain}/records");
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
                        let del_url = format!("{BASE_URL}/domains/{domain}/records/{id}");
                        let _ = http::delete(&del_url, &[("Authorization", &auth)]);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

fn auth_header(user: &str, pass: &str) -> String {
    let creds = base64::encode_std(format!("{user}:{pass}").as_bytes());
    format!("Basic {creds}")
}

fn record_id(v: &Value) -> Option<String> {
    v.get("id").and_then(|i| {
        if i.is_string() { i.as_str().map(|s| s.to_string()) }
        else if i.is_u64() { i.as_u64().map(|n| n.to_string()) }
        else if i.is_i64() { i.as_i64().map(|n| n.to_string()) }
        else { None }
    })
}
