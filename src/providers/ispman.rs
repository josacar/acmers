use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Ispman {
    base_url: String,
    basic_auth: String,
}

impl DnsProvider for Ispman {
    fn slug() -> &'static str { "ispman" }
    fn env_vars() -> &'static [&'static str] { &["ISPMAN_Url", "ISPMAN_Username", "ISPMAN_Password"] }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let base_url = env.get("ISPMAN_Url")
            .ok_or_else(|| Error::Config("ISPMAN_Url required".into()))?
            .trim_end_matches('/').to_string();
        let user = env.get("ISPMAN_Username")
            .ok_or_else(|| Error::Config("ISPMAN_Username required".into()))?.clone();
        let pass = env.get("ISPMAN_Password")
            .ok_or_else(|| Error::Config("ISPMAN_Password required".into()))?.clone();
        let creds = format!("{user}:{pass}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Ispman { base_url, basic_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let rec_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let body = serde_json::json!({
            "type": "TXT",
            "name": rec_name,
            "content": value,
            "ttl": 120,
        });
        let url = format!("{}/api/dns/domains/{}/records", self.base_url, domain);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("ispman add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("ispman add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let rec_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let list_url = format!("{}/api/dns/domains/{}/records", self.base_url, domain);
        let resp = match http::get(&list_url, headers) {
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
                        let del_url = format!("{}/api/dns/domains/{}/records/{}", self.base_url, domain, id);
                        let _ = http::delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}
