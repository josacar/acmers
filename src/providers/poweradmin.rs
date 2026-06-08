use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Poweradmin {
    username: String,
    password: String,
    hostname: String,
}

impl DnsProvider for Poweradmin {
    fn slug() -> &'static str {
        "poweradmin"
    }

    fn env_vars() -> &'static [&'static str] {
        &["POWERADMIN_Username", "POWERADMIN_Password", "POWERADMIN_Hostname"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("POWERADMIN_Username")
            .ok_or_else(|| Error::Config("POWERADMIN_Username required".into()))?
            .clone();
        let password = env.get("POWERADMIN_Password")
            .ok_or_else(|| Error::Config("POWERADMIN_Password required".into()))?
            .clone();
        let hostname = env.get("POWERADMIN_Hostname")
            .ok_or_else(|| Error::Config("POWERADMIN_Hostname required".into()))?
            .clone();
        Ok(Box::new(Poweradmin { username, password, hostname }))
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
        let url = format!("https://{}/api/v1/domains/{}/records", self.hostname, domain);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("poweradmin add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("poweradmin add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let auth = auth_header(&self.username, &self.password);
        let rec_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);

        let list_url = format!("https://{}/api/v1/domains/{}/records", self.hostname, domain);
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
                        let del_url = format!("https://{}/api/v1/domains/{}/records/{}", self.hostname, domain, id);
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
