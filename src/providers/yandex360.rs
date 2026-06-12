use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::json as j;
use crate::providers::{DnsProvider, ProviderResult};

const API_BASE: &str = "https://api360.yandex.net/directory/v1";

pub struct Yandex360 {
    access_token: String,
    org_id: String,
}

impl DnsProvider for Yandex360 {
    fn slug() -> &'static str {
        "yandex360"
    }

    fn env_vars() -> &'static [&'static str] {
        &["YANDEX360_ACCESS_TOKEN"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let access_token = env.get("YANDEX360_ACCESS_TOKEN")
            .ok_or_else(|| Error::Config("YANDEX360_ACCESS_TOKEN required".into()))?
            .clone();
        let org_id = env.get("YANDEX360_ORG_ID")
            .cloned()
            .unwrap_or_default();
        Ok(Box::new(Yandex360 { access_token, org_id }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let org_id = self.resolve_org_id()?;
        let root_domain = self.find_root_domain(&org_id, name)?;
        let sub = name.strip_suffix(&format!(".{root_domain}")).unwrap_or(name);
        let url = format!("{API_BASE}/org/{org_id}/domains/{root_domain}/dns");
        let auth = format!("OAuth {}", self.access_token);
        let body = serde_json::to_vec(&serde_json::json!({
            "name": sub,
            "type": "TXT",
            "ttl": 60,
            "text": value,
        })).unwrap();
        let resp = http::post(&url, &body, "application/json",
            &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("Yandex360 add TXT: {e}")))?;
        if resp.status >= 400 || !resp.body.contains("recordId") {
            return Err(Error::Provider(format!("Yandex360 add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let org_id = match self.resolve_org_id() {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let root_domain = match self.find_root_domain(&org_id, name) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };
        let auth = format!("OAuth {}", self.access_token);
        let url = format!("{API_BASE}/org/{org_id}/domains/{root_domain}/dns?perPage=100");
        let resp = match http::get(&url, &[("Authorization", &auth)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if !resp.body.contains(value) {
            return Ok(());
        }
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(records) = j::get_array(&v, &["records"]) {
            for record in records {
                if record.get("text").and_then(|t| t.as_str()) == Some(value) {
                    if let Some(id) = record.get("recordId").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        let del_url = format!("{API_BASE}/org/{org_id}/domains/{root_domain}/dns/{id}");
                        let _ = http::delete(&del_url, &[("Authorization", &auth)]);
                    }
                }
            }
        }
        Ok(())
    }
}

impl Yandex360 {
    fn resolve_org_id(&self) -> Result<String, Error> {
        if !self.org_id.is_empty() {
            return Ok(self.org_id.clone());
        }
        let auth = format!("OAuth {}", self.access_token);
        let resp = http::get(&format!("{API_BASE}/org"), &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("Yandex360 list orgs: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Yandex360 orgs: {e}")))?;
        let orgs = j::get_array(&v, &["organizations"])
            .ok_or_else(|| Error::Provider("Yandex360: no organizations found".into()))?;
        let org = orgs.first()
            .ok_or_else(|| Error::Provider("Yandex360: no organizations".into()))?;
        let id = org.get("id")
            .and_then(|i| {
                if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
            })
            .ok_or_else(|| Error::Provider("Yandex360: no org id".into()))?;
        Ok(id)
    }

    fn find_root_domain(&self, org_id: &str, domain: &str) -> Result<String, Error> {
        let auth = format!("OAuth {}", self.access_token);
        let url = format!("{API_BASE}/org/{org_id}/domains");
        let resp = http::get(&url, &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("Yandex360 list domains: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Yandex360 domains: {e}")))?;
        let domains = j::get_array(&v, &["domains"])
            .ok_or_else(|| Error::Provider("Yandex360: no domains".into()))?;
        for d in domains {
            if let Some(name) = d.get("name").and_then(|n| n.as_str()) {
                if domain == name || domain.ends_with(&format!(".{name}")) {
                    return Ok(name.to_string());
                }
            }
        }
        Err(Error::Provider(format!("Yandex360: domain not found for {domain}")))
    }
}
