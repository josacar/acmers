use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Yc {
    folder_id: String,
    oauth_token: String,
}

impl DnsProvider for Yc {
    fn slug() -> &'static str {
        "yc"
    }

    fn env_vars() -> &'static [&'static str] {
        &["YC_KeyID", "YC_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let folder_id = env.get("YC_KeyID")
            .ok_or_else(|| Error::Config("YC_KeyID required".into()))?
            .clone();
        let oauth_token = env.get("YC_Secret")
            .ok_or_else(|| Error::Config("YC_Secret required".into()))?
            .clone();
        Ok(Box::new(Yc { folder_id, oauth_token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = self.get_iam_token()?;
        let headers: &[(&str, &str)] = &[("Authorization", &token)];
        let zone_id = self.resolve_zone(domain, headers)?;
        let body = serde_json::to_vec(&serde_json::json!({
            "replacements": [{
                "name": name,
                "type": "TXT",
                "ttl": 120,
                "data": [value],
            }]
        })).unwrap();
        let url = format!("https://dns.api.cloud.yandex.net/dns/v1/zones/{}:upsertRecordSets", zone_id);
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("YC add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("YC add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let token = match self.get_iam_token() {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };
        let headers: &[(&str, &str)] = &[("Authorization", &token)];
        let zone_id = match self.resolve_zone(domain, headers) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let body = serde_json::to_vec(&serde_json::json!({
            "deletions": [{
                "name": name,
                "type": "TXT",
            }]
        })).unwrap();
        let url = format!("https://dns.api.cloud.yandex.net/dns/v1/zones/{}:upsertRecordSets", zone_id);
        let _ = http::post(&url, &body, "application/json", headers);
        Ok(())
    }
}

impl Yc {
    fn get_iam_token(&self) -> Result<String, Error> {
        let body = serde_json::to_vec(&serde_json::json!({
            "yandexPassportOauthToken": self.oauth_token,
        })).unwrap();
        let resp = http::post("https://iam.api.cloud.yandex.net/iam/v1/tokens", &body, "application/json", &[])
            .map_err(|e| Error::Provider(format!("YC IAM auth: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("YC IAM auth: {e}")))?;
        let token = v.get("iamToken").and_then(|t| t.as_str())
            .ok_or_else(|| Error::Provider(format!("YC IAM auth: no token in response: {}", resp.body)))?;
        Ok(format!("Bearer {token}"))
    }

    fn resolve_zone(&self, domain: &str, headers: &[(&str, &str)]) -> Result<String, Error> {
        let url = format!("https://dns.api.cloud.yandex.net/dns/v1/zones?folderId={}", self.folder_id);
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("YC list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("YC zones: {e}")))?;
        let domain_dot = format!("{domain}.");
        if let Some(zones) = v.get("dnsZones").and_then(|z| z.as_array()) {
            for zone in zones {
                let zone_name = zone.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if zone_name == domain || zone_name == domain_dot.as_str() {
                    if let Some(id) = zone.get("id").and_then(|i| i.as_str()) {
                        return Ok(id.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}
