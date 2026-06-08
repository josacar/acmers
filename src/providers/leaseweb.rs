use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Leaseweb {
    api_key: String,
}

impl DnsProvider for Leaseweb {
    fn slug() -> &'static str {
        "leaseweb"
    }

    fn env_vars() -> &'static [&'static str] {
        &["LEASEWEB_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("LEASEWEB_API_KEY")
            .ok_or_else(|| Error::Config("LEASEWEB_API_KEY required".into()))?.clone();
        Ok(Box::new(Leaseweb { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("X-Lsw-Auth", &self.api_key)];
        let domain_name = self.resolve_domain(domain, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "name": name,
            "type": "TXT",
            "ttl": 120,
            "resourceRecords": [{"content": format!("\"{}\"", value)}],
        })).unwrap();
        let url = format!("https://api.leaseweb.com/dns/v1/domains/{}/resourceRecordSets", domain_name);
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("Leaseweb add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Leaseweb add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("X-Lsw-Auth", &self.api_key)];
        let domain_name = match self.resolve_domain(domain, headers) {
            Ok(n) => n,
            Err(_) => return Ok(()),
        };
        let list_url = format!("https://api.leaseweb.com/dns/v1/domains/{}/resourceRecordSets/{}/TXT", domain_name, name);
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if resp.status >= 400 {
            return Ok(());
        }
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let match_content = format!("\"{}\"", value);
        let mut found = false;
        if let Some(records) = v.get("resourceRecords").and_then(|r| r.as_array()) {
            for record in records {
                if record.get("content").and_then(|c| c.as_str()) == Some(&match_content) {
                    found = true;
                    break;
                }
            }
        }
        if found {
            let del_url = format!("https://api.leaseweb.com/dns/v1/domains/{}/resourceRecordSets/{}/TXT", domain_name, name);
            let _ = http::delete(&del_url, headers);
        }
        Ok(())
    }
}

impl Leaseweb {
    fn resolve_domain(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let resp = http::get("https://api.leaseweb.com/dns/v1/domains", headers)
            .map_err(|e| Error::Provider(format!("Leaseweb list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Leaseweb domains: {e}")))?;
        if let Some(domains) = v.pointer("/_embedded/domains").and_then(|d| d.as_array()) {
            for d in domains {
                if let Some(nm) = d.get("name").and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        return Ok(nm.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
