use std::collections::HashMap;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Opnsense {
    base: String,
    auth_header: String,
}

impl DnsProvider for Opnsense {
    fn slug() -> &'static str {
        "opnsense"
    }

    fn env_vars() -> &'static [&'static str] {
        &["OPNSENSE_API_KEY", "OPNSENSE_API_SECRET", "OPNSENSE_HOST"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let host = env.get("OPNSENSE_HOST")
            .ok_or_else(|| Error::Config("OPNSENSE_HOST required".into()))?
            .clone();
        let api_key = env.get("OPNSENSE_API_KEY")
            .ok_or_else(|| Error::Config("OPNSENSE_API_KEY required".into()))?
            .clone();
        let api_secret = env.get("OPNSENSE_API_SECRET")
            .ok_or_else(|| Error::Config("OPNSENSE_API_SECRET required".into()))?
            .clone();

        let port = env.get("OPNSENSE_PORT").map(|s| s.as_str()).unwrap_or("443");
        let base = build_base_url(&host, port);
        let creds = base64::encode_std(format!("{api_key}:{api_secret}").as_bytes());
        let auth_header = format!("Basic {creds}");

        Ok(Box::new(Opnsense { base, auth_header }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (domain_id, zone, host) = self.get_root(name)?;
        let uuid = self.find_existing_record(&zone, &host, value)?;

        let body = serde_json::json!({
            "record": {
                "enabled": "1",
                "domain": domain_id,
                "name": host,
                "type": "TXT",
                "value": value,
            }
        });

        let url = if let Some(ref uuid) = uuid {
            format!("{}/record/setRecord/{}", self.base, uuid)
        } else {
            format!("{}/record/addRecord", self.base)
        };

        let resp = http::post(
            &url,
            &serde_json::to_vec(&body).unwrap(),
            "application/json",
            &[("Authorization", &self.auth_header)],
        ).map_err(|e| Error::Provider(format!("opnsense add record: {e}")))?;

        if !resp.body.contains("\"result\":\"saved\"") {
            return Err(Error::Provider(format!("opnsense add record failed: {}", resp.body)));
        }

        let _ = http::post(
            &format!("{}/service/reconfigure", self.base),
            b"{}",
            "application/json",
            &[("Authorization", &self.auth_header)],
        );

        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (_domain_id, zone, host) = match self.get_root(name) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let uuid = match self.find_existing_record(&zone, &host, value) {
            Ok(Some(u)) => u,
            Ok(None) => return Ok(()),
            Err(_) => return Ok(()),
        };

        let url = format!("{}/record/delRecord/{}", self.base, uuid);
        let resp = http::post(
            &url,
            b"{}",
            "application/json",
            &[("Authorization", &self.auth_header)],
        ).map_err(|e| Error::Provider(format!("opnsense delete record: {e}")))?;

        if resp.body.contains("\"result\":\"deleted\"") {
            let _ = http::post(
                &format!("{}/service/reconfigure", self.base),
                b"{}",
                "application/json",
                &[("Authorization", &self.auth_header)],
            );
        }

        Ok(())
    }
}

impl Opnsense {
    fn api_get(&self, endpoint: &str) -> Result<String, Error> {
        let url = format!("{}{}", self.base, endpoint);
        let resp = http::get(&url, &[("Authorization", &self.auth_header)])
            .map_err(|e| Error::Provider(format!("opnsense GET {endpoint}: {e}")))?;
        Ok(resp.body)
    }

    fn get_root(&self, fulldomain: &str) -> Result<(String, String, String), Error> {
        let body = self.api_get("/domain/searchPrimaryDomain")?;
        let v: Value = serde_json::from_str(&body)
            .map_err(|e| Error::Json(format!("opnsense searchPrimaryDomain: {e}")))?;

        let rows = v.get("rows").and_then(|r| r.as_array())
            .ok_or_else(|| Error::Provider("opnsense: no rows in domain search response".into()))?;

        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 1..parts.len().saturating_sub(1) {
            let candidate = parts[i..].join(".");
            for row in rows {
                let enabled = row.get("enabled").and_then(|e| e.as_str()).unwrap_or("");
                let r#type = row.get("type").and_then(|t| t.as_str()).unwrap_or("");
                let domainname = row.get("domainname").and_then(|d| d.as_str()).unwrap_or("");
                if enabled == "1" && r#type == "primary" && domainname == candidate {
                    let uuid = row.get("uuid").and_then(|u| u.as_str())
                        .ok_or_else(|| Error::Provider("opnsense: no uuid in domain row".into()))?
                        .to_string();
                    let host = parts[..i].join(".");
                    return Ok((uuid, candidate, host));
                }
            }
        }

        Err(Error::Provider(format!("opnsense: zone not found for {fulldomain}")))
    }

    fn find_existing_record(&self, domain: &str, name: &str, value: &str) -> Result<Option<String>, Error> {
        let body = self.api_get("/record/searchRecord")?;
        let v: Value = serde_json::from_str(&body)
            .map_err(|e| Error::Json(format!("opnsense searchRecord: {e}")))?;

        let rows = v.get("rows").and_then(|r| r.as_array());
        if let Some(rows) = rows {
            for row in rows {
                let row_domain = row.get("domain").and_then(|d| d.as_str()).unwrap_or("");
                let pct_domain = row.get("%domain").and_then(|d| d.as_str()).unwrap_or("");
                let row_name = row.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let row_type = row.get("type").and_then(|t| t.as_str()).unwrap_or("");
                let row_value = row.get("value").and_then(|v| v.as_str()).unwrap_or("");
                if (row_domain == domain || pct_domain == domain)
                    && row_name == name
                    && row_type == "TXT"
                    && row_value == value
                {
                    if let Some(uuid) = row.get("uuid").and_then(|u| u.as_str()) {
                        return Ok(Some(uuid.to_string()));
                    }
                }
            }
        }
        Ok(None)
    }
}

fn build_base_url(host: &str, port: &str) -> String {
    let host = host.trim().trim_end_matches('/');
    if host.contains("://") {
        format!("{host}/api/bind")
    } else if port == "443" {
        format!("https://{host}/api/bind")
    } else {
        format!("https://{host}:{port}/api/bind")
    }
}
