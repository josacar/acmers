use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct GandiLivedns {
    token: String,
}

impl DnsProvider for GandiLivedns {
    fn slug() -> &'static str {
        "gandi_livedns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["GANDI_LIVEDNS_TOKEN"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("GANDI_LIVEDNS_TOKEN")
            .ok_or_else(|| Error::Config("GANDI_LIVEDNS_TOKEN required".into()))?
            .clone();
        Ok(Box::new(GandiLivedns { token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let fqdn = self.resolve_zone(domain, &auth)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "rrset_type": "TXT",
            "rrset_name": name,
            "rrset_ttl": 120,
            "rrset_values": [value],
        })).unwrap();
        let url = format!("https://api.gandi.net/v5/livedns/domains/{fqdn}/records");
        let resp = http::post(&url, &body, "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("Gandi add TXT: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Gandi response: {e}")))?;
        if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
            if resp.status >= 400 {
                return Err(Error::Provider(format!("Gandi add TXT: {msg}")));
            }
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let fqdn = match self.resolve_zone(domain, &auth) {
            Ok(f) => f,
            Err(_) => return Ok(()),
        };
        let del_url = format!("https://api.gandi.net/v5/livedns/domains/{fqdn}/records/{name}/TXT");
        let _ = http::delete(&del_url, &[("Authorization", &auth)]);
        Ok(())
    }
}

impl GandiLivedns {
    fn resolve_zone(&self, domain: &str, auth: &str) -> Result<String, Error> {
        let resp = http::get("https://api.gandi.net/v5/livedns/domains", &[("Authorization", auth)])
            .map_err(|e| Error::Provider(format!("Gandi list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Gandi domains: {e}")))?;
        if let Some(fqdns) = v.as_array() {
            for f in fqdns {
                if let Some(fqdn) = f.as_str() {
                    if domain == fqdn || domain.ends_with(&format!(".{fqdn}")) {
                        return Ok(fqdn.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
