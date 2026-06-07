use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Mydevil {
    basic_auth: String,
}

impl DnsProvider for Mydevil {
    fn slug() -> &'static str {
        "mydevil"
    }

    fn env_vars() -> &'static [&'static str] {
        &["MYDEVIL_Username", "MYDEVIL_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("MYDEVIL_Username")
            .ok_or_else(|| Error::Config("MYDEVIL_Username required".into()))?.clone();
        let password = env.get("MYDEVIL_Password")
            .ok_or_else(|| Error::Config("MYDEVIL_Password required".into()))?.clone();
        let creds = format!("{username}:{password}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Mydevil { basic_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let url = format!("https://api.mydevil.net/dns/add/{domain}");
        let form_data = format!("type=TXT&name={}&data={}&ttl=120",
            urlencoding(name), urlencoding(value));
        let resp = http::post(&url, form_data.as_bytes(), "application/x-www-form-urlencoded",
            &[("Authorization", &self.basic_auth)])
            .map_err(|e| Error::Provider(format!("mydevil add TXT: {e}")))?;
        if resp.status != 200 {
            return Err(Error::Provider(format!("mydevil add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let record_id = self.find_record_id(domain, name, value)?;
        if let Some(id) = record_id {
            let url = format!("https://api.mydevil.net/dns/delete/{domain}/{id}");
            http::post(&url, b"", "application/json",
                &[("Authorization", &self.basic_auth)])
                .map_err(|e| Error::Provider(format!("mydevil delete TXT: {e}")))?;
        }
        Ok(())
    }
}

impl Mydevil {
    fn find_record_id(&self, domain: &str, name: &str, value: &str) -> Result<Option<String>, Error> {
        let url = format!("https://api.mydevil.net/dns/list/{domain}");
        let resp = http::get(&url, &[("Authorization", &self.basic_auth)])
            .map_err(|e| Error::Provider(format!("mydevil list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("mydevil records: {e}")))?;
        let records: Option<&Vec<Value>> = v.as_array()
            .or_else(|| v.get("records").and_then(|r| r.as_array()))
            .or_else(|| v.get("data").and_then(|r| r.as_array()));
        if let Some(arr) = records {
            for r in arr {
                if r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && r.get("name").and_then(|n| n.as_str()) == Some(name)
                    && r.get("data").and_then(|d| d.as_str()) == Some(value)
                {
                    return Ok(value_to_string(r.get("id").or_else(|| r.get("record_id"))));
                }
            }
        }
        Ok(None)
    }
}

fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn value_to_string(v: Option<&Value>) -> Option<String> {
    v.and_then(|v| v.as_str().map(|s| s.to_string())
        .or_else(|| v.as_i64().map(|i| i.to_string())))
}
