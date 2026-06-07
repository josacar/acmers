use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Bluehost {
    hostname: String,
    username: String,
    api_token: String,
}

impl DnsProvider for Bluehost {
    fn slug() -> &'static str {
        "bh"
    }

    fn env_vars() -> &'static [&'static str] {
        &["BH_Key", "BH_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let hostname = env.get("BH_Key")
            .ok_or_else(|| Error::Config("BH_Key required".into()))?.clone();
        let username = hostname.split('.').next().unwrap_or("").to_string();
        let api_token = env.get("BH_Secret")
            .ok_or_else(|| Error::Config("BH_Secret required".into()))?.clone();
        Ok(Box::new(Bluehost { hostname, username, api_token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let full_name = format!("{name}.{domain}.");
        let url = format!(
            "https://{}:2083/json-api/cpanel?cpanel_jsonapi_user={}&cpanel_jsonapi_apiversion=2&cpanel_jsonapi_module=ZoneEdit&cpanel_jsonapi_func=add_zone_record&domain={}&name={}&type=TXT&txtdata=\"{}\"&ttl=60",
            self.hostname, self.username, domain, full_name, value
        );
        let auth = format!("cpanel {}:{}", self.username, self.api_token);
        let resp = http::get(&url, &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("bluehost add TXT: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("bluehost response: {e}")))?;
        if let Some(data) = v.get("cpanelresult").and_then(|r| r.get("data")).and_then(|d| d.as_array()) {
            if let Some(first) = data.first() {
                if let Some(result) = first.get("result") {
                    if result.get("status").and_then(|s| s.as_i64()) == Some(1) {
                        return Ok(());
                    }
                    if let Some(msg) = result.get("statusmsg").and_then(|m| m.as_str()) {
                        return Err(Error::Provider(format!("bluehost add TXT: {msg}")));
                    }
                }
            }
        }
        if let Some(err) = v.get("cpanelresult").and_then(|r| r.get("error")).and_then(|e| e.as_str()) {
            return Err(Error::Provider(format!("bluehost add TXT: {err}")));
        }
        Err(Error::Provider(format!("bluehost add TXT: unexpected response: {}", resp.body)))
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        match self.find_record_line(domain, name, value) {
            Ok(Some(line)) => {
                let url = format!(
                    "https://{}:2083/json-api/cpanel?cpanel_jsonapi_user={}&cpanel_jsonapi_apiversion=2&cpanel_jsonapi_module=ZoneEdit&cpanel_jsonapi_func=remove_zone_record&domain={}&line={}",
                    self.hostname, self.username, domain, line
                );
                let auth = format!("cpanel {}:{}", self.username, self.api_token);
                http::get(&url, &[("Authorization", &auth)]).ok();
                Ok(())
            }
            Ok(None) => Ok(()),
            Err(e) => {
                eprintln!("warning: bluehost cleanup failed: {e}");
                Ok(())
            }
        }
    }
}

impl Bluehost {
    fn find_record_line(&self, domain: &str, name: &str, value: &str) -> Result<Option<String>, Error> {
        let name_full = format!("{name}.{domain}.");
        let url = format!(
            "https://{}:2083/json-api/cpanel?cpanel_jsonapi_user={}&cpanel_jsonapi_apiversion=2&cpanel_jsonapi_module=ZoneEdit&cpanel_jsonapi_func=fetchzone&domain={}",
            self.hostname, self.username, domain
        );
        let auth = format!("cpanel {}:{}", self.username, self.api_token);
        let resp = http::get(&url, &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("bluehost fetchzone: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("bluehost fetchzone: {e}")))?;
        if let Some(data) = v.get("cpanelresult").and_then(|r| r.get("data")).and_then(|d| d.as_array()) {
            for record in data {
                let rname = record.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let rtype = record.get("type").and_then(|t| t.as_str()).unwrap_or("");
                if rtype == "TXT" && rname == name_full {
                    let rdata = record.get("txtdata").and_then(|t| t.as_str()).unwrap_or("");
                    let clean_data = rdata.trim_matches('"');
                    if clean_data == value {
                        if let Some(line) = record.get("line") {
                            let line_str = if line.is_number() {
                                line.as_i64().map(|n| n.to_string())
                            } else {
                                line.as_str().map(|s| s.to_string())
                            };
                            if let Some(line_str) = line_str {
                                return Ok(Some(line_str));
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }
}
