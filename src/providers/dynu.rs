use std::collections::HashMap;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.dynu.com/v2";

pub struct Dynu {
    auth_header: String,
}

impl DnsProvider for Dynu {
    fn slug() -> &'static str {
        "dynu"
    }

    fn env_vars() -> &'static [&'static str] {
        &["Dynu_ClientId", "Dynu_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let client_id = env.get("Dynu_ClientId")
            .ok_or_else(|| Error::Config("Dynu_ClientId required".into()))?
            .clone();
        let secret = env.get("Dynu_Secret")
            .ok_or_else(|| Error::Config("Dynu_Secret required".into()))?
            .clone();
        let creds = base64::encode_std(format!("{client_id}:{secret}").as_bytes());
        let auth_header = format!("Basic {creds}");
        Ok(Box::new(Dynu { auth_header }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = self.resolve_domain(domain)?;
        let short_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);

        let body = serde_json::json!({
            "nodeName": short_name,
            "recordType": "TXT",
            "textData": value,
            "ttl": 120,
            "state": true,
        });
        let url = format!("{BASE_URL}/dns/{domain_id}/record");
        let headers: &[(&str, &str)] = &[("Authorization", &self.auth_header)];
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("Dynu create record: {e}")))?;

        if resp.status >= 300 {
            return Err(Error::Provider(format!("Dynu create record: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = match self.resolve_domain(domain) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let short_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);

        let record_id = match self.find_record(&domain_id, short_name, value) {
            Ok(Some(id)) => id,
            Ok(None) => return Ok(()),
            Err(_) => return Ok(()),
        };

        let url = format!("{BASE_URL}/dns/{domain_id}/record/{record_id}");
        let headers: &[(&str, &str)] = &[("Authorization", &self.auth_header)];
        let _ = http::delete(&url, headers);
        Ok(())
    }
}

impl Dynu {
    fn resolve_domain(&self, domain: &str) -> Result<String, Error> {
        let url = format!("{BASE_URL}/dns");
        let headers: &[(&str, &str)] = &[("Authorization", &self.auth_header)];
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("Dynu list domains: {e}")))?;

        if resp.status >= 300 {
            return Err(Error::Provider(format!("Dynu list domains: {} {}", resp.status, resp.body)));
        }

        let v: serde_json::Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Dynu domains: {e}")))?;

        if let Some(domains) = v.get("domains").and_then(|d| d.as_array()) {
            for d in domains {
                let dname = d.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if domain == dname || domain.ends_with(&format!(".{dname}")) {
                    if let Some(id) = d.get("id").and_then(|i| i.as_i64()).map(|i| i.to_string()) {
                        return Ok(id);
                    }
                }
            }
        }
        Err(Error::Provider(format!("domain not found: {domain}")))
    }

    fn find_record(&self, domain_id: &str, short_name: &str, value: &str) -> Result<Option<String>, Error> {
        let url = format!("{BASE_URL}/dns/{domain_id}");
        let headers: &[(&str, &str)] = &[("Authorization", &self.auth_header)];
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("Dynu get records: {e}")))?;

        if resp.status >= 300 {
            return Ok(None);
        }

        let v: serde_json::Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Dynu records: {e}")))?;

        if let Some(records) = v.get("dnsRecords").and_then(|r| r.as_array()) {
            for record in records {
                let rname = record.get("nodeName").and_then(|n| n.as_str()).unwrap_or("");
                let rval = record.get("textData").and_then(|v| v.as_str()).unwrap_or("");
                let rtype = record.get("recordType").and_then(|t| t.as_str()).unwrap_or("");
                if rtype == "TXT" && rname == short_name && rval == value {
                    if let Some(id) = record.get("id").and_then(|i| i.as_i64()).map(|i| i.to_string()) {
                        return Ok(Some(id));
                    }
                }
            }
        }
        Ok(None)
    }
}
