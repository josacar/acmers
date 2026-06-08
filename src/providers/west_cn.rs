use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.west.cn/API/v2";

pub struct WestCn {
    username: String,
    apikey: String,
}

impl DnsProvider for WestCn {
    fn slug() -> &'static str { "west_cn" }
    fn env_vars() -> &'static [&'static str] { &["WEST_Username", "WEST_Key"] }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("WEST_Username")
            .ok_or_else(|| Error::Config("WEST_Username required".into()))?.clone();
        let apikey = env.get("WEST_Key")
            .ok_or_else(|| Error::Config("WEST_Key required".into()))?.clone();
        Ok(Box::new(WestCn { username, apikey }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let form = format!(
            "act=dnsrec.add&username={}&apikey={}&domain={}&hostname={}&record_type=TXT&record_value={}",
            urlencode(&self.username),
            urlencode(&self.apikey),
            urlencode(name),
            urlencode(name),
            urlencode(value),
        );
        let url = format!("{BASE_URL}/domain/dns/");
        let resp = http::post(&url, form.as_bytes(), "application/x-www-form-urlencoded", &[])
            .map_err(|e| Error::Provider(format!("west_cn add TXT: {e}")))?;
        if resp.status >= 400 || !resp.body.contains("success") {
            return Err(Error::Provider(format!("west_cn add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let list_form = format!(
            "act=dnsrec.list&username={}&apikey={}&domain={}&hostname={}&record_type=TXT",
            urlencode(&self.username),
            urlencode(&self.apikey),
            urlencode(name),
            urlencode(name),
        );
        let url = format!("{BASE_URL}/domain/dns/");
        let resp = match http::post(&url, list_form.as_bytes(), "application/x-www-form-urlencoded", &[]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if resp.body.contains("no records") {
            return Ok(());
        }
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records = v.get("data").and_then(|d| d.as_array())
            .or_else(|| v.as_array());
        if let Some(records) = records {
            for record in records {
                let matches_value = record.get("record_value").and_then(|v| v.as_str()) == Some(value)
                    || record.get("value").and_then(|v| v.as_str()) == Some(value);
                let matches_type = record.get("record_type").and_then(|t| t.as_str()) == Some("TXT")
                    || record.get("type").and_then(|t| t.as_str()) == Some("TXT");
                if matches_value && matches_type {
                    if let Some(id) = record.get("record_id").and_then(|i| {
                        if i.is_u64() { Some(i.as_u64().unwrap().to_string()) }
                        else if i.is_string() { Some(i.as_str().unwrap().to_string()) }
                        else { None }
                    }) {
                        let del_form = format!(
                            "act=dnsrec.remove&username={}&apikey={}&domain={}&hostname={}&record_id={}",
                            urlencode(&self.username),
                            urlencode(&self.apikey),
                            urlencode(name),
                            urlencode(name),
                            urlencode(&id),
                        );
                        let _ = http::post(&url, del_form.as_bytes(), "application/x-www-form-urlencoded", &[]);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}
