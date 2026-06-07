use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Infomaniak {
    token: String,
}

impl DnsProvider for Infomaniak {
    fn slug() -> &'static str {
        "infomaniak"
    }

    fn env_vars() -> &'static [&'static str] {
        &["INFOMANIAK_ACCESS_TOKEN"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("INFOMANIAK_ACCESS_TOKEN")
            .ok_or_else(|| Error::Config("INFOMANIAK_ACCESS_TOKEN required".into()))?.clone();
        Ok(Box::new(Infomaniak { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = self.resolve_zone(domain)?;
        let url = format!("https://api.infomaniak.com/1/domain/{domain_id}/dns/record");
        let body = serde_json::json!({
            "source": name,
            "type": "TXT",
            "target": value,
            "ttl": 120,
        });
        let auth = format!("Bearer {}", self.token);
        http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("infomaniak add TXT: {e}")))?;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = self.resolve_zone(domain)?;
        let record_id = self.find_record_id(&domain_id, name, value)?;
        if let Some(id) = record_id {
            let url = format!("https://api.infomaniak.com/1/domain/{domain_id}/dns/record/{id}");
            let auth = format!("Bearer {}", self.token);
            http::delete(&url, &[("Authorization", &auth)])
                .map_err(|e| Error::Provider(format!("infomaniak delete TXT: {e}")))?;
        }
        Ok(())
    }
}

impl Infomaniak {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let auth = format!("Bearer {}", self.token);
        let resp = http::get("https://api.infomaniak.com/1/domain", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("infomaniak list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("infomaniak domains: {e}")))?;
        let domains = v.get("data").and_then(|d| d.as_array())
            .or_else(|| v.as_array());
        if let Some(arr) = domains {
            for d in arr {
                if let Some(nm) = d.get("customer_name").and_then(|n| n.as_str()) {
                    if domain == nm || domain.ends_with(&format!(".{nm}")) {
                        if let Some(id) = value_to_string(d.get("id")) {
                            return Ok(id);
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }

    fn find_record_id(&self, domain_id: &str, name: &str, value: &str) -> Result<Option<String>, Error> {
        let auth = format!("Bearer {}", self.token);
        let url = format!("https://api.infomaniak.com/1/domain/{domain_id}/dns/record");
        let resp = http::get(&url, &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("infomaniak list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("infomaniak records: {e}")))?;
        let records = v.get("data").and_then(|d| d.as_array())
            .or_else(|| v.as_array());
        if let Some(arr) = records {
            for r in arr {
                if r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && r.get("source").and_then(|s| s.as_str()) == Some(name)
                    && r.get("target").and_then(|t| t.as_str()) == Some(value)
                {
                    return Ok(value_to_string(r.get("id")));
                }
            }
        }
        Ok(None)
    }
}

fn value_to_string(v: Option<&Value>) -> Option<String> {
    v.and_then(|v| v.as_str().map(|s| s.to_string())
        .or_else(|| v.as_i64().map(|i| i.to_string())))
}
