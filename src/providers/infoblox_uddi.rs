use std::collections::HashMap;

use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct InfobloxUddi {
    api_key: String,
    portal: String,
}

fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 3);
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

impl InfobloxUddi {
    fn api_base(&self) -> String {
        format!("https://{}/api/ddi/v1", self.portal)
    }

    fn api_get(&self, endpoint: &str) -> Result<http::Response, Error> {
        let url = format!("{}/{}", self.api_base(), endpoint);
        let auth = format!("Token {}", self.api_key);
        http::get(&url, &[("Authorization", &auth), ("Content-Type", "application/json")])
            .map_err(|e| Error::Provider(format!("infoblox_uddi GET: {e}")))
    }

    fn api_post(&self, endpoint: &str, body: &[u8]) -> Result<http::Response, Error> {
        let url = format!("{}/{}", self.api_base(), endpoint);
        let auth = format!("Token {}", self.api_key);
        http::post(&url, body, "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("infoblox_uddi POST: {e}")))
    }

    fn api_delete(&self, endpoint: &str) -> Result<http::Response, Error> {
        let url = format!("{}/{}", self.api_base(), endpoint);
        let auth = format!("Token {}", self.api_key);
        http::delete(&url, &[("Authorization", &auth), ("Content-Type", "application/json")])
            .map_err(|e| Error::Provider(format!("infoblox_uddi DELETE: {e}")))
    }

    fn find_zone(&self, fulldomain: &str) -> Result<(String, String, String), Error> {
        let domain_no_acme = fulldomain
            .strip_prefix("_acme-challenge.")
            .unwrap_or(fulldomain);

        let parts: Vec<&str> = domain_no_acme.split('.').collect();

        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                continue;
            }

            let filter = format!(
                "fqdn%20eq%20'{}.'%20or%20fqdn%20eq%20'{}'",
                percent_encode(&h),
                percent_encode(&h),
            );
            let endpoint = format!("dns/auth_zone?_filter={}", filter);

            let resp = self.api_get(&endpoint)?;

            if resp.status == 401 {
                return Err(Error::Provider("infoblox_uddi: authentication failed. Check Infoblox_UDDI_Key.".into()));
            }

            if resp.status >= 400 {
                continue;
            }

            let v: Value = serde_json::from_str(&resp.body)
                .map_err(|e| Error::Json(format!("infoblox_uddi parse zone: {e}")))?;

            if let Some(results) = v.get("results").and_then(|r| r.as_array()) {
                for zone in results {
                    if let Some(id) = zone.get("id").and_then(|i| i.as_str()) {
                        if id.starts_with("dns/auth_zone/") {
                            let sub_domain = if h == fulldomain {
                                String::new()
                            } else {
                                let cut_len = fulldomain.len() - h.len() - 1;
                                fulldomain[..cut_len].to_string()
                            };
                            return Ok((id.to_string(), sub_domain, h));
                        }
                    }
                }
            }
        }

        Err(Error::Provider(format!("infoblox_uddi: zone not found for {fulldomain}")))
    }
}

impl DnsProvider for InfobloxUddi {
    fn slug() -> &'static str {
        "infoblox_uddi"
    }

    fn env_vars() -> &'static [&'static str] {
        &["Infoblox_UDDI_Key", "Infoblox_Portal"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("Infoblox_UDDI_Key")
            .ok_or_else(|| Error::Config("Infoblox_UDDI_Key required".into()))?;
        let portal = env.get("Infoblox_Portal")
            .ok_or_else(|| Error::Config("Infoblox_Portal required".into()))?;
        Ok(Box::new(InfobloxUddi {
            api_key: api_key.clone(),
            portal: portal.clone(),
        }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone_id, sub_domain, _domain_name) = self.find_zone(name)?;

        let body = serde_json::json!({
            "type": "TXT",
            "name_in_zone": sub_domain,
            "zone": zone_id,
            "ttl": 120,
            "inheritance_sources": {
                "ttl": {
                    "action": "override"
                }
            },
            "rdata": {
                "text": value
            }
        });

        let resp = self.api_post("dns/record", &serde_json::to_vec(&body).unwrap())?;

        if resp.body.contains(value) {
            return Ok(());
        }

        if let Ok(v) = serde_json::from_str::<Value>(&resp.body) {
            if let Some(err) = v.get("error").or_else(|| v.get("Error")) {
                let err_str = err.to_string();
                if err_str.contains("already exists") || err_str.contains("duplicate") {
                    return Ok(());
                }
                return Err(Error::Provider(format!("infoblox_uddi add TXT: {}", resp.body)));
            }
        }

        if resp.status < 400 {
            return Ok(());
        }

        Err(Error::Provider(format!("infoblox_uddi add TXT: HTTP {} {}", resp.status, resp.body)))
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone_id, sub_domain, _domain_name) = match self.find_zone(name) {
            Ok(z) => z,
            Err(e) => {
                eprintln!("warning: infoblox_uddi cleanup: {e}");
                return Ok(());
            }
        };

        let filter = format!(
            "type%20eq%20'TXT'%20and%20name_in_zone%20eq%20'{}'%20and%20zone%20eq%20'{}'%20and%20rdata.text%20eq%20'{}'",
            percent_encode(&sub_domain),
            percent_encode(&zone_id),
            percent_encode(value),
        );
        let endpoint = format!("dns/record?_filter={}", filter);

        let resp = match self.api_get(&endpoint) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("warning: infoblox_uddi cleanup: {e}");
                return Ok(());
            }
        };

        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        if v.get("results").and_then(|r| r.as_array()).is_none() {
            return Ok(());
        }

        let record_id = v.get("results")
            .and_then(|r| r.as_array())
            .and_then(|arr| arr.first())
            .and_then(|r| r.get("id"))
            .and_then(|i| i.as_str())
            .map(|s| s.to_string());

        let record_id = match record_id {
            Some(id) => id,
            None => return Ok(()),
        };

        let record_uuid = record_id.rsplit('/').next().unwrap_or(&record_id).to_string();

        let resp = match self.api_delete(&format!("dns/record/{}", record_uuid)) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("warning: infoblox_uddi cleanup: {e}");
                return Ok(());
            }
        };

        if resp.status >= 400 {
            eprintln!("warning: infoblox_uddi cleanup: HTTP {} {}", resp.status, resp.body);
            return Ok(());
        }

        Ok(())
    }
}
