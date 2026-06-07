use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Rage4 {
    basic_auth: String,
}

impl DnsProvider for Rage4 {
    fn slug() -> &'static str {
        "rage4"
    }

    fn env_vars() -> &'static [&'static str] {
        &["RAGE4_Key", "RAGE4_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let key = env.get("RAGE4_Key")
            .ok_or_else(|| Error::Config("RAGE4_Key required".into()))?
            .clone();
        let secret = env.get("RAGE4_Secret")
            .ok_or_else(|| Error::Config("RAGE4_Secret required".into()))?
            .clone();
        let creds = format!("{key}:{secret}");
        let basic_auth = format!("Basic {}", base64::encode(creds.as_bytes()));
        Ok(Box::new(Rage4 { basic_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let url = format!("https://rage4.com/rapi/createrecord/?id={zone_id}&name={name}&content={value}&type=TXT&ttl=120");
        let resp = http::get(&url, &[("Authorization", &self.basic_auth)])
            .map_err(|e| Error::Provider(format!("rage4 create: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("rage4 create: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let record_id = match self.find_record(&zone_id, name, value) {
            Some(id) => id,
            None => return Ok(()),
        };
        let url = format!("https://rage4.com/rapi/deleterecord/?id={record_id}");
        http::get(&url, &[("Authorization", &self.basic_auth)]).ok();
        Ok(())
    }
}

impl Rage4 {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let url = "https://rage4.com/rapi/getdomains/";
        let resp = http::get(url, &[("Authorization", &self.basic_auth)])
            .map_err(|e| Error::Provider(format!("rage4 list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("rage4 parse: {e}")))?;
        if let Some(domains) = v.as_array() {
            for d in domains {
                if let Some(dname) = d.get("domain").and_then(|n| n.as_str()) {
                    if domain == dname || domain.ends_with(&format!(".{dname}")) {
                        if let Some(id) = d.get("id").and_then(|i| i.as_str()) {
                            return Ok(id.to_string());
                        }
                        if let Some(id_num) = d.get("id").and_then(|i| i.as_i64()) {
                            return Ok(id_num.to_string());
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("rage4 zone not found for {domain}")))
    }

    fn find_record(&self, zone_id: &str, name: &str, value: &str) -> Option<String> {
        let url = format!("https://rage4.com/rapi/getrecords/?id={zone_id}");
        let resp = http::get(&url, &[("Authorization", &self.basic_auth)]).ok()?;
        let v: Value = serde_json::from_str(&resp.body).ok()?;
        if let Some(records) = v.as_array() {
            for rec in records {
                let rec_type = rec.get("type").and_then(|t| t.as_str());
                let rec_name = rec.get("name").and_then(|n| n.as_str());
                let rec_content = rec.get("content").and_then(|c| c.as_str());
                if rec_type == Some("TXT") && rec_name == Some(name) && rec_content == Some(value) {
                    if let Some(id) = rec.get("id").and_then(|i| i.as_str()) {
                        return Some(id.to_string());
                    }
                    if let Some(id_num) = rec.get("id").and_then(|i| i.as_i64()) {
                        return Some(id_num.to_string());
                    }
                }
            }
        }
        None
    }
}
