use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.tele3.cz/api/v1";

pub struct Tele3 {
    key: String,
    secret: String,
}

impl DnsProvider for Tele3 {
    fn slug() -> &'static str { "tele3" }
    fn env_vars() -> &'static [&'static str] { &["TELE3_Key", "TELE3_Secret"] }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let key = env.get("TELE3_Key")
            .ok_or_else(|| Error::Config("TELE3_Key required".into()))?.clone();
        let secret = env.get("TELE3_Secret")
            .ok_or_else(|| Error::Config("TELE3_Secret required".into()))?.clone();
        Ok(Box::new(Tele3 { key, secret }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("sso-key {}:{}", self.key, self.secret);
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
            .map_err(|e| Error::Provider(format!("tele3 add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("tele3 add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let auth = format!("sso-key {}:{}", self.key, self.secret);
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
                    if let Some(id) = record.get("id").and_then(|i| if i.is_string() { i.as_str() } else { None })
                        .or_else(|| record.get("record_id").and_then(|i| i.as_str()))
                    {
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
