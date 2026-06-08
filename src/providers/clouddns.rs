use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::json as j;
use crate::providers::{DnsProvider, ProviderResult};

const API_BASE: &str = "https://admin.vshosting.cloud/clouddns";
const LOGIN_URL: &str = "https://admin.vshosting.cloud/api/public/auth/login";

pub struct Clouddns {
    token: String,
    client_id: String,
}

impl DnsProvider for Clouddns {
    fn slug() -> &'static str {
        "clouddns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["CLOUDDNS_CLIENT_ID", "CLOUDDNS_EMAIL", "CLOUDDNS_PASSWORD"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let client_id = env.get("CLOUDDNS_CLIENT_ID")
            .ok_or_else(|| Error::Config("CLOUDDNS_CLIENT_ID required".into()))?.clone();
        let email = env.get("CLOUDDNS_EMAIL")
            .ok_or_else(|| Error::Config("CLOUDDNS_EMAIL required".into()))?.clone();
        let password = env.get("CLOUDDNS_PASSWORD")
            .ok_or_else(|| Error::Config("CLOUDDNS_PASSWORD required".into()))?.clone();

        let login_body = serde_json::json!({
            "email": email,
            "password": password,
        });
        let resp = http::post(
            LOGIN_URL,
            &serde_json::to_vec(&login_body).unwrap(),
            "application/json",
            &[],
        ).map_err(|e| Error::Provider(format!("clouddns login: {e}")))?;

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("clouddns login: {e}")))?;

        let token = j::get_string_required(&v, &["accessToken"])?.to_string();

        Ok(Box::new(Clouddns { token, client_id }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = self.resolve_domain(domain)?;
        let url = format!("{API_BASE}/record-txt");
        let body = serde_json::json!({
            "type": "TXT",
            "name": format!("{name}."),
            "value": value,
            "domainId": domain_id,
        });
        let auth = format!("Bearer {}", self.token);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[
            ("Authorization", &auth),
        ]).map_err(|e| Error::Provider(format!("clouddns add TXT: {e}")))?;

        if resp.status >= 400 {
            return Err(Error::Provider(format!("clouddns add TXT: HTTP {} {}", resp.status, resp.body)));
        }

        self.publish(&domain_id)?;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = match self.resolve_domain(domain) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("warning: clouddns cleanup zone not found: {e}");
                return Ok(());
            }
        };
        let auth = format!("Bearer {}", self.token);

        let detail_url = format!("{API_BASE}/domain/{domain_id}");
        let resp = match http::get(&detail_url, &[("Authorization", &auth)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };

        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let record_name = format!("{name}.");
        if let Some(records) = v.get("lastDomainRecordList").and_then(|d| d.as_array()) {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(record_name.as_str())
                    && record.get("value").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| i.as_str()) {
                        let del_url = format!("{API_BASE}/record/{id}");
                        let _ = http::delete(&del_url, &[("Authorization", &auth)]);
                        self.publish(&domain_id)?;
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Clouddns {
    fn publish(&self, domain_id: &str) -> ProviderResult {
        let auth = format!("Bearer {}", self.token);
        let url = format!("{API_BASE}/domain/{domain_id}/publish");
        let body = serde_json::json!({"soaTtl": 300});
        http::put(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[
            ("Authorization", &auth),
        ]).map_err(|e| Error::Provider(format!("clouddns publish: {e}")))?;
        Ok(())
    }

    fn resolve_domain(&self, domain: &str) -> Result<String, Error> {
        let auth = format!("Bearer {}", self.token);

        let search_body = serde_json::json!({
            "search": [{"name": "clientId", "operator": "eq", "value": self.client_id}]
        });
        let resp = http::post(
            &format!("{API_BASE}/domain/search"),
            &serde_json::to_vec(&search_body).unwrap(),
            "application/json",
            &[("Authorization", &auth)],
        ).map_err(|e| Error::Provider(format!("clouddns domain search: {e}")))?;

        if resp.status >= 400 {
            return Err(Error::Provider(format!("clouddns domain search: HTTP {} {}", resp.status, resp.body)));
        }

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("clouddns domain search: {e}")))?;

        let domains = v.as_array()
            .or_else(|| v.get("data").and_then(|d| d.as_array()))
            .ok_or_else(|| Error::Provider("clouddns domain search: unexpected response".into()))?;

        let domain_names: Vec<String> = domains.iter()
            .filter_map(|d| d.get("domainName").and_then(|n| n.as_str()).map(|s| s.to_string()))
            .collect();

        let mut domain_root: Option<String> = None;
        let mut slice = domain.to_string();
        loop {
            let search = format!("{slice}.");
            if domain_names.iter().any(|n| n == &search) {
                domain_root = Some(slice);
                break;
            }
            if let Some(pos) = slice.find('.') {
                slice = slice[pos + 1..].to_string();
            } else {
                break;
            }
        }

        let domain_root = domain_root
            .ok_or_else(|| Error::Provider(format!("domain not found for {domain}")))?;

        let search_body = serde_json::json!({
            "search": [
                {"name": "clientId", "operator": "eq", "value": self.client_id},
                {"name": "domainName", "operator": "eq", "value": format!("{domain_root}.")}
            ]
        });
        let resp = http::post(
            &format!("{API_BASE}/domain/search"),
            &serde_json::to_vec(&search_body).unwrap(),
            "application/json",
            &[("Authorization", &auth)],
        ).map_err(|e| Error::Provider(format!("clouddns domain search: {e}")))?;

        if resp.status >= 400 {
            return Err(Error::Provider(format!("clouddns domain search: HTTP {} {}", resp.status, resp.body)));
        }

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("clouddns domain search: {e}")))?;

        let domains = v.as_array()
            .or_else(|| v.get("data").and_then(|d| d.as_array()));

        if let Some(domains) = domains {
            for d in domains {
                if let Some(id) = d.get("id").and_then(|i| i.as_str()) {
                    return Ok(id.to_string());
                }
            }
        }

        Err(Error::Provider(format!("domain id not found for {domain_root}")))
    }
}
