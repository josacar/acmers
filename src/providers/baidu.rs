use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://bcd.baidubce.com";

pub struct Baidu {
    auth: String,
}

impl DnsProvider for Baidu {
    fn slug() -> &'static str {
        "baidu"
    }

    fn env_vars() -> &'static [&'static str] {
        &["BAIDU_Key", "BAIDU_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let key = env.get("BAIDU_Key")
            .ok_or_else(|| Error::Config("BAIDU_Key required".into()))?
            .clone();
        let secret = env.get("BAIDU_Secret")
            .ok_or_else(|| Error::Config("BAIDU_Secret required".into()))?
            .clone();
        let creds = format!("{key}:{secret}");
        let auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Baidu { auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = self.resolve_zone(domain)
            .map_err(|_| Error::Provider("Baidu Cloud requires complex API signing. Not yet implemented.".into()))?;
        let body = serde_json::json!({
            "type": "TXT",
            "name": name,
            "content": value,
            "ttl": 120,
        });
        let url = format!("{BASE_URL}/v1/dns/zone/{zone}/record");
        let headers: &[(&str, &str)] = &[("Authorization", &self.auth)];
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|_| Error::Provider("Baidu Cloud requires complex API signing. Not yet implemented.".into()))?;
        if resp.status >= 400 {
            return Err(Error::Provider("Baidu Cloud requires complex API signing. Not yet implemented.".into()));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = match self.resolve_zone(domain) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let list_url = format!("{BASE_URL}/v1/dns/zone/{zone}/record");
        let headers: &[(&str, &str)] = &[("Authorization", &self.auth)];
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
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("{BASE_URL}/v1/dns/zone/{zone}/record/{id}");
                        let _ = http::delete(&del_url, headers);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Baidu {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let url = format!("{BASE_URL}/v1/dns/zone");
        let headers: &[(&str, &str)] = &[("Authorization", &self.auth)];
        let body = serde_json::json!({});
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("baidu list zones: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("baidu list zones: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("baidu parse zones: {e}")))?;
        let zones = v.as_array()
            .or_else(|| v.get("zones").and_then(|z| z.as_array()))
            .or_else(|| v.get("data").and_then(|d| d.as_array()));
        if let Some(zones) = zones {
            for z in zones {
                if let Some(nm) = z.get("name").or_else(|| z.get("domain")).and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        if let Some(id) = z.get("id").and_then(|i| {
                            if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                        }).or_else(|| z.get("name").and_then(|n| n.as_str()).map(|s| s.to_string())) {
                            return Ok(id);
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("baidu: zone not found for {domain}")))
    }
}
