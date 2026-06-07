use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Miab {
    basic_auth: String,
    server: String,
}

impl DnsProvider for Miab {
    fn slug() -> &'static str {
        "miab"
    }

    fn env_vars() -> &'static [&'static str] {
        &["MIAB_Username", "MIAB_Password", "MIAB_Server"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("MIAB_Username")
            .ok_or_else(|| Error::Config("MIAB_Username required".into()))?
            .clone();
        let password = env.get("MIAB_Password")
            .ok_or_else(|| Error::Config("MIAB_Password required".into()))?
            .clone();
        let server = env.get("MIAB_Server")
            .ok_or_else(|| Error::Config("MIAB_Server required".into()))?
            .clone();
        let creds = format!("{username}:{password}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Miab { basic_auth, server }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let body = serde_json::json!({
            "qname": name,
            "rtype": "TXT",
            "value": value,
        });
        let url = format!("https://{}/admin/dns/custom/{domain}", self.server);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("miab add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("miab add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let list_url = format!("https://{}/admin/dns/custom/{domain}", self.server);
        let resp = match http::get(&list_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(records) = v.as_array() {
            for r in records {
                if r.get("rtype").and_then(|t| t.as_str()) == Some("TXT")
                    && r.get("qname").and_then(|n| n.as_str()) == Some(name)
                    && r.get("value").and_then(|v| v.as_str()) == Some(value)
                {
                    let del_body = serde_json::json!({
                        "qname": name,
                        "rtype": "TXT",
                        "value": "",
                    });
                    let del_url = format!("https://{}/admin/dns/custom/{domain}/{name}", self.server);
                    let _ = http::post(&del_url, &serde_json::to_vec(&del_body).unwrap(), "application/json", headers);
                    return Ok(());
                }
            }
        }
        Ok(())
    }
}
