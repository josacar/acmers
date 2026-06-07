use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Scaleway {
    api_token: String,
    project_id: Option<String>,
}

impl DnsProvider for Scaleway {
    fn slug() -> &'static str {
        "scaleway"
    }

    fn env_vars() -> &'static [&'static str] {
        &["SCALEWAY_API_TOKEN", "SCALEWAY_PROJECT_ID"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_token = env.get("SCALEWAY_API_TOKEN")
            .ok_or_else(|| Error::Config("SCALEWAY_API_TOKEN required".into()))?.clone();
        Ok(Box::new(Scaleway {
            api_token,
            project_id: env.get("SCALEWAY_PROJECT_ID").cloned(),
        }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let url = format!("https://api.scaleway.com/domain/v2beta1/dns-zones/{zone_id}/records");
        let body = serde_json::json!({
            "changes": [{
                "add": {
                    "records": [{
                        "data": value,
                        "name": name,
                        "ttl": 120,
                        "type": "TXT",
                    }]
                }
            }]
        });
        let resp = http::patch(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("X-Auth-Token", &self.api_token)])
            .map_err(|e| Error::Provider(format!("scaleway add TXT: {e}")))?;
        if resp.status >= 300 {
            return Err(Error::Provider(format!("scaleway add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let url = format!("https://api.scaleway.com/domain/v2beta1/dns-zones/{zone_id}/records");
        let body = serde_json::json!({
            "changes": [{
                "delete": {
                    "records": [{
                        "data": value,
                        "name": name,
                        "type": "TXT",
                    }]
                }
            }]
        });
        http::patch(&url, &serde_json::to_vec(&body).unwrap(), "application/json",
            &[("X-Auth-Token", &self.api_token)])
            .map_err(|e| Error::Provider(format!("scaleway delete TXT: {e}")))?;
        Ok(())
    }
}

impl Scaleway {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let mut url = "https://api.scaleway.com/domain/v2beta1/dns-zones".to_string();
        if let Some(ref pid) = self.project_id {
            url.push_str(&format!("?project_id={pid}"));
        }
        let resp = http::get(&url, &[("X-Auth-Token", &self.api_token)])
            .map_err(|e| Error::Provider(format!("scaleway list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("scaleway zones: {e}")))?;
        let zones = v.get("dns_zones").and_then(|z| z.as_array())
            .or_else(|| v.as_array());
        if let Some(arr) = zones {
            for z in arr {
                if let Some(nm) = z.get("domain").and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        if let Some(id) = z.get("id").and_then(|i| i.as_str()) {
                            return Ok(id.to_string());
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
