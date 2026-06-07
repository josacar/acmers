use std::collections::HashMap;

use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const TOKEN_URL: &str = "https://login.microsoftonline.com";
const BASE_URL: &str = "https://management.azure.com/subscriptions";

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
        let (rg_name, zone_name) = self.resolve_zone(domain, &auth)?;

        let record_name = if name.ends_with(&format!(".{zone_name}")) {
            name.strip_suffix(&format!(".{zone_name}")).unwrap_or(name)
        } else {
            name
        };

        let body = serde_json::json!({
            "properties": {
                "TTL": 60,
                "TXTRecords": [{"value": [value]}]
            }
        });
        let url = format!(
            "{BASE_URL}/{sub}/resourceGroups/{rg}/providers/Microsoft.Network/dnsZones/{zone}/TXT/{record_name}?api-version=2018-05-01",
            sub = self.subscription_id, rg = rg_name, zone = zone_name
        );
        let resp = http::put(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("Azure add TXT: {e}")))?;

        if resp.status >= 300 {
            return Err(Error::Provider(format!("Azure add TXT: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let token = match get_token(&self.tenant_id, &self.app_id, &self.client_secret) {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };
        let auth = format!("Bearer {token}");
        let (rg_name, zone_name) = match self.resolve_zone(domain, &auth) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };

        let record_name = if name.ends_with(&format!(".{zone_name}")) {
            name.strip_suffix(&format!(".{zone_name}")).unwrap_or(name)
        } else {
            name
        };

        let url = format!(
            "{BASE_URL}/{sub}/resourceGroups/{rg}/providers/Microsoft.Network/dnsZones/{zone}/TXT/{record_name}?api-version=2018-05-01",
            sub = self.subscription_id, rg = rg_name, zone = zone_name
        );
        let _ = http::delete(&url, &[("Authorization", &auth)]);
        Ok(())
    }
}

impl Azure {
    fn resolve_zone(&self, domain: &str, auth: &str) -> Result<(String, String), Error> {
        let groups_url = format!("{BASE_URL}/{sub}/resourceGroups?api-version=2021-04-01", sub = self.subscription_id);
        let resp = http::get(&groups_url, &[("Authorization", auth)])
            .map_err(|e| Error::Provider(format!("Azure list groups: {e}")))?;

        if resp.status >= 300 {
            return Err(Error::Provider(format!("Azure list groups: {} {}", resp.status, resp.body)));
        }

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Azure groups: {e}")))?;

        if let Some(groups) = v.get("value").and_then(|g| g.as_array()) {
            for group in groups {
                if let Some(rg_name) = group.get("name").and_then(|n| n.as_str()) {
                    let zones_url = format!(
                        "{BASE_URL}/{sub}/resourceGroups/{rg}/providers/Microsoft.Network/dnsZones?api-version=2018-05-01",
                        sub = self.subscription_id, rg = rg_name
                    );
                    let zones_resp = match http::get(&zones_url, &[("Authorization", auth)]) {
                        Ok(r) => r,
                        Err(_) => continue,
                    };

                    if zones_resp.status >= 300 {
                        continue;
                    }

                    let zones_v: Value = match serde_json::from_str(&zones_resp.body) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    if let Some(zones) = zones_v.get("value").and_then(|z| z.as_array()) {
                        for zone in zones {
                            if let Some(name) = zone.get("name").and_then(|n| n.as_str()) {
                                if domain == name || domain.ends_with(&format!(".{name}")) {
                                    return Ok((rg_name.to_string(), name.to_string()));
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}

fn get_token(tenant_id: &str, app_id: &str, client_secret: &str) -> Result<String, Error> {
    let url = format!("{TOKEN_URL}/{tenant_id}/oauth2/v2.0/token");
    let body = format!("grant_type=client_credentials&client_id={app_id}&client_secret={client_secret}&scope=https://management.azure.com/.default");
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
