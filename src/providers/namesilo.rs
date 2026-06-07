use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Namesilo {
    api_key: String,
}

impl DnsProvider for Namesilo {
    fn slug() -> &'static str {
        "namesilo"
    }

    fn env_vars() -> &'static [&'static str] {
        &["Namesilo_Key"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("Namesilo_Key")
            .ok_or_else(|| Error::Config("Namesilo_Key required".into()))?
            .clone();
        Ok(Box::new(Namesilo { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let url = format!(
            "https://www.namesilo.com/api/dnsAddRecord?version=1&type=json&key={}&domain={}&rrtype=TXT&rrhost={}&rrvalue={}&rrttl=7207",
            urlencode(&self.api_key),
            urlencode(domain),
            urlencode(name),
            urlencode(value),
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("Namesilo add TXT: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Namesilo response: {e}")))?;
        if v.get("reply").and_then(|r| r.get("code").and_then(|c| c.as_i64())) != Some(300) {
            let detail = v.get("reply").and_then(|r| r.get("detail").and_then(|d| d.as_str())).unwrap_or("unknown");
            return Err(Error::Provider(format!("Namesilo add TXT: {detail}")));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let record_id = match self.find_record_id(domain, name, value) {
            Some(id) => id,
            None => return Ok(()),
        };
        let url = format!(
            "https://www.namesilo.com/api/dnsDeleteRecord?version=1&type=json&key={}&domain={}&rrid={}",
            urlencode(&self.api_key),
            urlencode(domain),
            urlencode(&record_id),
        );
        let _ = http::get(&url, &[]);
        Ok(())
    }
}

impl Namesilo {
    fn find_record_id(&self, domain: &str, name: &str, value: &str) -> Option<String> {
        let url = format!(
            "https://www.namesilo.com/api/dnsListRecords?version=1&type=json&key={}&domain={}",
            urlencode(&self.api_key),
            urlencode(domain),
        );
        let resp = http::get(&url, &[]).ok()?;
        let v: Value = serde_json::from_str(&resp.body).ok()?;
        let records = v.get("reply")?.get("resource_record")?.as_array()?;
        for record in records {
            if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                && record.get("host").and_then(|h| h.as_str()) == Some(name)
                && record.get("value").and_then(|v| v.as_str()) == Some(value)
            {
                return record.get("record_id").and_then(|r| r.as_str()).map(|s| s.to_string())
                    .or_else(|| record.get("record_id").and_then(|r| r.as_i64()).map(|i| i.to_string()));
            }
        }
        None
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
