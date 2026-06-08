use std::collections::HashMap;

use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const TOKEN_URL: &str = "https://login.microsoftonline.com";
const BASE_URL: &str = "https://management.azure.com/subscriptions";
const API_VERSION: &str = "2017-09-01";

pub struct Azure {
    subscription_id: String,
    tenant_id: String,
    app_id: String,
    client_secret: String,
}

impl DnsProvider for Azure {
    fn slug() -> &'static str {
        "azure"
    }

    fn env_vars() -> &'static [&'static str] {
        &["AZUREDNS_SUBSCRIPTIONID", "AZUREDNS_TENANTID", "AZUREDNS_APPID", "AZUREDNS_CLIENTSECRET"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let subscription_id = env.get("AZUREDNS_SUBSCRIPTIONID")
            .ok_or_else(|| Error::Config("AZUREDNS_SUBSCRIPTIONID required".into()))?
            .clone();
        let tenant_id = env.get("AZUREDNS_TENANTID")
            .ok_or_else(|| Error::Config("AZUREDNS_TENANTID required".into()))?
            .clone();
        let app_id = env.get("AZUREDNS_APPID")
            .ok_or_else(|| Error::Config("AZUREDNS_APPID required".into()))?
            .clone();
        let client_secret = env.get("AZUREDNS_CLIENTSECRET")
            .ok_or_else(|| Error::Config("AZUREDNS_CLIENTSECRET required".into()))?
            .clone();
        Ok(Box::new(Azure { subscription_id, tenant_id, app_id, client_secret }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = get_token(&self.tenant_id, &self.app_id, &self.client_secret)?;
        let auth = format!("Bearer {token}");
        let (domain_id, sub_domain) = self.resolve_zone(domain, &auth)?;

        let url = format!(
            "https://management.azure.com{domain_id}/TXT/{sub_domain}?api-version={API_VERSION}",
        );

        let mut records = vec![serde_json::json!({"value": [value]})];

        let existing = http::get(&url, &[("Authorization", &auth)]);
        if let Ok(ref resp) = existing {
            if resp.status == 200 {
                if let Ok(v) = serde_json::from_str::<Value>(&resp.body) {
                    if let Some(arr) = v.pointer("/properties/TXTRecords").and_then(|a| a.as_array()) {
                        for entry in arr {
                            if let Some(val_arr) = entry.get("value").and_then(|v| v.as_array()) {
                                for val in val_arr {
                                    records.push(serde_json::json!({"value": [val]}));
                                }
                            }
                        }
                    }
                }
            }
        }

        let body = serde_json::json!({
            "properties": {
                "TTL": 10,
                "TXTRecords": records
            }
        });
        let resp = http::put(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("Azure add TXT: {e}")))?;

        if resp.status >= 300 {
            return Err(Error::Provider(format!("Azure add TXT: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = match get_token(&self.tenant_id, &self.app_id, &self.client_secret) {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };
        let auth = format!("Bearer {token}");
        let (domain_id, sub_domain) = match self.resolve_zone(domain, &auth) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };

        let url = format!(
            "https://management.azure.com{domain_id}/TXT/{sub_domain}?api-version={API_VERSION}",
        );

        let resp = match http::get(&url, &[("Authorization", &auth)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };

        if resp.status != 200 {
            return Ok(());
        }

        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let mut remaining = Vec::new();
        if let Some(arr) = v.pointer("/properties/TXTRecords").and_then(|a| a.as_array()) {
            for entry in arr {
                if let Some(val_arr) = entry.get("value").and_then(|v| v.as_array()) {
                    for val in val_arr {
                        if let Some(s) = val.as_str() {
                            if s != value {
                                remaining.push(serde_json::json!({"value": [s]}));
                            }
                        }
                    }
                }
            }
        }

        if remaining.is_empty() {
            let _ = http::delete(&url, &[("Authorization", &auth)]);
        } else {
            let body = serde_json::json!({
                "properties": {
                    "TTL": 10,
                    "TXTRecords": remaining
                }
            });
            let _ = http::put(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)]);
        }
        Ok(())
    }
}

impl Azure {
    fn resolve_zone(&self, domain: &str, auth: &str) -> Result<(String, String), Error> {
        let url = format!(
            "{BASE_URL}/{sub}/providers/Microsoft.Network/dnszones?$top=500&api-version={API_VERSION}",
            sub = self.subscription_id,
        );
        let resp = http::get(&url, &[("Authorization", auth)])
            .map_err(|e| Error::Provider(format!("Azure list zones: {e}")))?;

        if resp.status >= 300 {
            return Err(Error::Provider(format!("Azure list zones: {} {}", resp.status, resp.body)));
        }

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Azure zones: {e}")))?;

        let zones = v.get("value").and_then(|z| z.as_array())
            .ok_or_else(|| Error::Provider("Azure zones: no value array".into()))?;

        let mut best_id = String::new();
        let mut best_sub = String::new();
        let mut best_len: usize = 0;

        for zone in zones {
            let zone_name = match zone.get("name").and_then(|n| n.as_str()) {
                Some(n) => n,
                None => continue,
            };
            let zone_id = match zone.get("id").and_then(|i| i.as_str()) {
                Some(i) => i,
                None => continue,
            };

            if domain == zone_name || domain.ends_with(&format!(".{zone_name}")) {
                let match_len = zone_name.len();
                if match_len > best_len {
                    best_len = match_len;
                    best_id = zone_id.to_string();
                    best_sub = if domain == zone_name {
                        "@".to_string()
                    } else {
                        domain[..domain.len() - zone_name.len() - 1].to_string()
                    };
                }
            }
        }

        if best_len == 0 {
            return Err(Error::Provider(format!("zone not found for {domain}")));
        }
        Ok((best_id, best_sub))
    }
}

fn get_token(tenant_id: &str, app_id: &str, client_secret: &str) -> Result<String, Error> {
    let url = format!("{TOKEN_URL}/{tenant_id}/oauth2/token");
    let body = format!("resource=https%3A%2F%2Fmanagement.core.windows.net%2F&client_id={app_id}&client_secret={client_secret}&grant_type=client_credentials");
    let resp = http::post(&url, body.as_bytes(), "application/x-www-form-urlencoded", &[])
        .map_err(|e| Error::Provider(format!("Azure auth: {e}")))?;

    if resp.status >= 300 {
        return Err(Error::Provider(format!("Azure auth: {} {}", resp.status, resp.body)));
    }

    let v: Value = serde_json::from_str(&resp.body)
        .map_err(|e| Error::Json(format!("Azure token: {e}")))?;
    v.get("access_token").and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| Error::Provider("no access_token in Azure response".into()))
}
