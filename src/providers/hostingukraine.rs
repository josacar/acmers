use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.hostingukraine.com.ua/api/v1";

pub struct Hostingukraine {
    uuid: String,
    token: String,
}

impl DnsProvider for Hostingukraine {
    fn slug() -> &'static str { "hostingukraine" }
    fn env_vars() -> &'static [&'static str] { &["HOSTINGUKRAINE_UUID", "HOSTINGUKRAINE_TOKEN"] }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let uuid = env.get("HOSTINGUKRAINE_UUID")
            .ok_or_else(|| Error::Config("HOSTINGUKRAINE_UUID required".into()))?.clone();
        let token = env.get("HOSTINGUKRAINE_TOKEN")
            .ok_or_else(|| Error::Config("HOSTINGUKRAINE_TOKEN required".into()))?.clone();
        Ok(Box::new(Hostingukraine { uuid, token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let rec_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let body = serde_json::json!({
            "type": "TXT",
            "name": rec_name,
            "content": value,
            "ttl": 120,
        });
        let url = format!("{BASE_URL}/domains/{domain}/dns-records");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("Authorization", &auth), ("X-Account-UUID", &self.uuid)])
            .map_err(|e| Error::Provider(format!("hostingukraine add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("hostingukraine add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let rec_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let list_url = format!("{BASE_URL}/domains/{domain}/dns-records");
        let resp = match http::get(&list_url, &[("Authorization", &auth), ("X-Account-UUID", &self.uuid)]) {
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
                    if let Some(id) = record.get("id").and_then(|i| if i.is_string() { i.as_str() } else { None })
                        .or_else(|| record.get("record_id").and_then(|i| i.as_str()))
                    {
                        let del_url = format!("{BASE_URL}/domains/{domain}/dns-records/{id}");
                        let _ = http::delete(&del_url, &[("Authorization", &auth), ("X-Account-UUID", &self.uuid)]);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}
