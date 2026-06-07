use std::collections::HashMap;

use serde_json::Value;

use crate::error::Error;
use crate::http;

use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://dns.googleapis.com/dns/v1/projects";

pub struct Gcloud {
    project: String,
    access_token: String,
}

impl DnsProvider for Gcloud {
    fn slug() -> &'static str {
        "gcloud"
    }

    fn env_vars() -> &'static [&'static str] {
        &["GCLOUD_PROJECT", "GCLOUD_SERVICE_ACCOUNT", "GCLOUD_ACCOUNT_TYPE"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let project = if let Some(p) = env.get("GCLOUD_PROJECT") {
            p.clone()
        } else if let Ok(p) = std::env::var("GCLOUD_PROJECT") {
            p
        } else {
            return Err(Error::Config("GCLOUD_PROJECT required".into()));
        };

        let access_token = get_access_token(env)?;

        Ok(Box::new(Gcloud { project, access_token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_name = self.resolve_zone(domain)?;
        let auth = format!("Bearer {}", self.access_token);

        let body = serde_json::json!({
            "additions": [{
                "name": format!("{name}."),
                "type": "TXT",
                "ttl": 60,
                "rrdatas": [value],
            }]
        });
        let url = format!("{BASE_URL}/{project}/managedZones/{zone_name}/changes", project = self.project);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("GCloud add TXT: {e}")))?;

        if resp.status >= 300 {
            return Err(Error::Provider(format!("GCloud add TXT: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_name = match self.resolve_zone(domain) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let auth = format!("Bearer {}", self.access_token);

        let body = serde_json::json!({
            "deletions": [{
                "name": format!("{name}."),
                "type": "TXT",
                "ttl": 60,
                "rrdatas": [value],
            }]
        });
        let url = format!("{BASE_URL}/{project}/managedZones/{zone_name}/changes", project = self.project);
        let _ = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)]);
        Ok(())
    }
}

impl Gcloud {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let auth = format!("Bearer {}", self.access_token);
        let url = format!("{BASE_URL}/{}/managedZones", self.project);
        let resp = http::get(&url, &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("GCloud list zones: {e}")))?;

        if resp.status >= 300 {
            return Err(Error::Provider(format!("GCloud list zones: {} {}", resp.status, resp.body)));
        }

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("GCloud zones: {e}")))?;

        let mut search = domain.to_string();
        if !search.ends_with('.') {
            search.push('.');
        }

        let mut best_len = 0;
        let mut best_name = String::new();

        if let Some(zones) = v.get("managedZones").and_then(|z| z.as_array()) {
            for zone in zones {
                if let Some(dns_name) = zone.get("dnsName").and_then(|n| n.as_str()) {
                    if search.ends_with(dns_name) && dns_name.len() > best_len {
                        best_len = dns_name.len();
                        if let Some(name) = zone.get("name").and_then(|n| n.as_str()) {
                            best_name = name.to_string();
                        }
                    }
                }
            }
        }

        if best_name.is_empty() {
            return Err(Error::Provider(format!("zone not found for {domain}")));
        }
        Ok(best_name)
    }
}

fn get_access_token(env: &HashMap<String, String>) -> Result<String, Error> {
    if let Some(token) = env.get("GCLOUD_ACCESS_TOKEN") {
        if !token.is_empty() {
            return Ok(token.clone());
        }
    }

    if let Ok(token) = std::env::var("GCLOUD_ACCESS_TOKEN") {
        if !token.is_empty() {
            return Ok(token);
        }
    }

    let url = "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";
    match http::get(url, &[("Metadata-Flavor", "Google")]) {
        Ok(resp) => {
            let v: Value = serde_json::from_str(&resp.body)
                .map_err(|e| Error::Json(format!("GCloud metadata token: {e}")))?;
            v.get("access_token").and_then(|t| t.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| Error::Provider("no access_token in metadata response".into()))
        }
        Err(_) => {
            Err(Error::Config(
                "GCloud requires GCLOUD_ACCESS_TOKEN env var or running on GCP with metadata server. \
                 Run: export GCLOUD_ACCESS_TOKEN=$(gcloud auth print-access-token)".into()
            ))
        }
    }
}
