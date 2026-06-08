use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.selectel.ru/domains";
const KEYSTONE_URL: &str = "https://cloud.api.selcloud.ru/identity/v3/auth/tokens";

pub struct Selectel {
    version: String,
    sl_key: Option<String>,
    sl_login_id: Option<String>,
    sl_project_name: Option<String>,
    sl_login_name: Option<String>,
    sl_pswd: Option<String>,
}

impl DnsProvider for Selectel {
    fn slug() -> &'static str {
        "selectel"
    }

    fn env_vars() -> &'static [&'static str] {
        &["SL_Ver", "SL_Key", "SL_Login_ID", "SL_Project_Name", "SL_Login_Name", "SL_Pswd", "SL_Expire"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let version = env.get("SL_Ver").cloned().unwrap_or_else(|| "v1".to_string());
        let sl_key = env.get("SL_Key").cloned();
        let sl_login_id = env.get("SL_Login_ID").cloned();
        let sl_project_name = env.get("SL_Project_Name").cloned();
        let sl_login_name = env.get("SL_Login_Name").cloned();
        let sl_pswd = env.get("SL_Pswd").cloned();

        if version == "v1" {
            sl_key.as_ref().ok_or_else(|| Error::Config("SL_Key required for v1".into()))?;
        } else if version == "v2" {
            if sl_login_name.is_none() || sl_login_id.is_none() || sl_project_name.is_none() || sl_pswd.is_none() {
                return Err(Error::Config("SL_Login_Name, SL_Login_ID, SL_Project_Name, SL_Pswd required for v2".into()));
            }
        } else {
            return Err(Error::Config(format!("Unsupported API version: {version}")));
        }

        Ok(Box::new(Selectel { version, sl_key, sl_login_id, sl_project_name, sl_login_name, sl_pswd }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = self.get_auth_token()?;
        let domain_id = self.get_root(domain, &token)?;

        if self.version == "v2" {
            self.add_txt_v2(&token, &domain_id, name, value)
        } else {
            self.add_txt_v1(&token, &domain_id, name, value)
        }
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = self.get_auth_token()?;
        let domain_id = self.get_root(domain, &token)?;

        if self.version == "v2" {
            self.remove_txt_v2(&token, &domain_id, name, value)
        } else {
            self.remove_txt_v1(&token, &domain_id, name, value)
        }
    }
}

impl Selectel {
    fn auth_header_name(&self) -> &str {
        if self.version == "v2" { "X-Auth-Token" } else { "X-Token" }
    }

    fn get_auth_token(&self) -> Result<String, Error> {
        if self.version == "v1" {
            Ok(self.sl_key.clone().unwrap())
        } else {
            self.get_keystone_token()
        }
    }

    fn get_keystone_token(&self) -> Result<String, Error> {
        let login_name = self.sl_login_name.as_ref().unwrap();
        let login_id = self.sl_login_id.as_ref().unwrap();
        let project_name = self.sl_project_name.as_ref().unwrap();
        let pswd = self.sl_pswd.as_ref().unwrap();

        let body = serde_json::json!({
            "auth": {
                "identity": {
                    "methods": ["password"],
                    "password": {
                        "user": {
                            "name": login_name,
                            "domain": { "name": login_id },
                            "password": pswd
                        }
                    }
                },
                "scope": {
                    "project": {
                        "name": project_name,
                        "domain": { "name": login_id }
                    }
                }
            }
        });

        let resp = http::post(KEYSTONE_URL, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("selectel keystone auth: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("selectel keystone auth: HTTP {} {}", resp.status, resp.body)));
        }
        resp.headers.get("x-subject-token").cloned()
            .ok_or_else(|| Error::Provider("selectel keystone auth: missing x-subject-token header".into()))
    }

    fn get_root(&self, domain: &str, token: &str) -> Result<String, Error> {
        if self.version == "v2" {
            self.get_root_v2(domain, token)
        } else {
            self.get_root_v1(domain, token)
        }
    }

    fn get_root_v1(&self, domain: &str, token: &str) -> Result<String, Error> {
        let hdr = self.auth_header_name();
        let url = format!("{BASE_URL}/v1/");
        let resp = http::get(&url, &[(hdr, token)])
            .map_err(|e| Error::Provider(format!("selectel list domains: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("selectel list domains: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Provider(format!("selectel list domains JSON: {e}")))?;
        let domains = v.as_array()
            .or_else(|| v.get("data").and_then(|d| d.as_array()))
            .ok_or_else(|| Error::Provider("selectel: unexpected list domains response".into()))?;

        let parts: Vec<&str> = domain.split('.').collect();
        for i in 1..parts.len() {
            let candidate = parts[i..].join(".");
            for d in domains {
                if d.get("name").and_then(|n| n.as_str()) == Some(&candidate) {
                    let id = d.get("id").and_then(|id| {
                        if id.is_string() { id.as_str().map(|s| s.to_string()) }
                        else if id.is_u64() { id.as_u64().map(|n| n.to_string()) }
                        else if id.is_i64() { id.as_i64().map(|n| n.to_string()) }
                        else { None }
                    }).ok_or_else(|| Error::Provider("selectel: domain id not found".into()))?;
                    return Ok(id);
                }
            }
        }
        Err(Error::Provider(format!("selectel: no matching domain found for {domain}")))
    }

    fn get_root_v2(&self, domain: &str, token: &str) -> Result<String, Error> {
        let hdr = self.auth_header_name();
        let url = format!("{BASE_URL}/v2/zones/");
        let resp = http::get(&url, &[(hdr, token)])
            .map_err(|e| Error::Provider(format!("selectel list zones: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("selectel list zones: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Provider(format!("selectel list zones JSON: {e}")))?;
        let zones = v.get("results").and_then(|r| r.as_array())
            .or_else(|| v.as_array())
            .ok_or_else(|| Error::Provider("selectel: unexpected list zones response".into()))?;

        let parts: Vec<&str> = domain.split('.').collect();
        for i in 1..parts.len() {
            let candidate = format!("{}.", parts[i..].join("."));
            for z in zones {
                if z.get("name").and_then(|n| n.as_str()) == Some(&candidate) {
                    let id = z.get("id").and_then(|id| id.as_str()).map(|s| s.to_string())
                        .ok_or_else(|| Error::Provider("selectel: zone id not found".into()))?;
                    return Ok(id);
                }
            }
        }
        Err(Error::Provider(format!("selectel: no matching zone found for {domain}")))
    }

    fn add_txt_v1(&self, token: &str, domain_id: &str, name: &str, value: &str) -> ProviderResult {
        let hdr = self.auth_header_name();
        let body = serde_json::json!({
            "type": "TXT",
            "ttl": 60,
            "name": name,
            "content": value,
        });
        let url = format!("{BASE_URL}/v1/{domain_id}/records/");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[(hdr, token)])
            .map_err(|e| Error::Provider(format!("selectel add TXT v1: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("selectel add TXT v1: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn add_txt_v2(&self, token: &str, zone_id: &str, name: &str, value: &str) -> ProviderResult {
        let hdr = self.auth_header_name();
        let quoted_value = format!("\"{}\"", value);
        let body = serde_json::json!({
            "type": "TXT",
            "ttl": 60,
            "name": format!("{}.", name),
            "records": [{"content": quoted_value}],
        });
        let url = format!("{BASE_URL}/v2/zones/{zone_id}/rrset/");
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[(hdr, token)])
            .map_err(|e| Error::Provider(format!("selectel add TXT v2: {e}")))?;

        if resp.body.contains("already_exists") {
            return self.patch_rrset_add_v2(token, zone_id, name, &quoted_value);
        }
        if resp.status >= 400 {
            return Err(Error::Provider(format!("selectel add TXT v2: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn patch_rrset_add_v2(&self, token: &str, zone_id: &str, name: &str, quoted_value: &str) -> ProviderResult {
        let hdr = self.auth_header_name();
        let list_url = format!("{BASE_URL}/v2/zones/{zone_id}/rrset/");
        let resp = http::get(&list_url, &[(hdr, token)])
            .map_err(|e| Error::Provider(format!("selectel list rrsets: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("selectel list rrsets: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Provider(format!("selectel list rrsets JSON: {e}")))?;
        let rrsets = v.get("results").and_then(|r| r.as_array())
            .or_else(|| v.as_array());

        let full_name = format!("{}.", name);
        if let Some(rrsets) = rrsets {
            for rrset in rrsets {
                if rrset.get("name").and_then(|n| n.as_str()) == Some(&full_name)
                    && rrset.get("type").and_then(|t| t.as_str()) == Some("TXT")
                {
                    let records = rrset.get("records").and_then(|r| r.as_array());
                    if let Some(records) = records {
                        for rec in records {
                            if rec.get("content").and_then(|c| c.as_str()) == Some(quoted_value) {
                                return Ok(());
                            }
                        }
                        let rrset_id = rrset.get("id").and_then(|id| id.as_str()).map(|s| s.to_string())
                            .ok_or_else(|| Error::Provider("selectel: rrset id not found".into()))?;
                        let ttl = rrset.get("ttl").and_then(|t| t.as_u64()).unwrap_or(60);
                        let mut new_records: Vec<Value> = records.clone();
                        new_records.push(serde_json::json!({"content": quoted_value}));
                        let patch_body = serde_json::json!({
                            "ttl": ttl,
                            "records": new_records,
                        });
                        let patch_url = format!("{BASE_URL}/v2/zones/{zone_id}/rrset/{rrset_id}");
                        let resp = http::patch(&patch_url, &serde_json::to_vec(&patch_body).unwrap(), "application/json", &[(hdr, token)])
                            .map_err(|e| Error::Provider(format!("selectel patch rrset: {e}")))?;
                        if resp.status >= 400 {
                            return Err(Error::Provider(format!("selectel patch rrset: HTTP {} {}", resp.status, resp.body)));
                        }
                        return Ok(());
                    }
                }
            }
        }
        Err(Error::Provider("selectel: could not find rrset to patch".into()))
    }

    fn remove_txt_v1(&self, token: &str, domain_id: &str, name: &str, value: &str) -> ProviderResult {
        let hdr = self.auth_header_name();
        let url = format!("{BASE_URL}/v1/{domain_id}/records/");
        let resp = match http::get(&url, &[(hdr, token)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let records = v.as_array()
            .or_else(|| v.get("data").and_then(|d| d.as_array()))
            .or_else(|| v.get("records").and_then(|r| r.as_array()));
        if let Some(records) = records {
            for record in records {
                if record.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && record.get("name").and_then(|n| n.as_str()) == Some(name)
                    && record.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record_id(record) {
                        let del_url = format!("{BASE_URL}/v1/{domain_id}/records/{id}");
                        let _ = http::delete(&del_url, &[(hdr, token)]);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }

    fn remove_txt_v2(&self, token: &str, zone_id: &str, name: &str, value: &str) -> ProviderResult {
        let hdr = self.auth_header_name();
        let url = format!("{BASE_URL}/v2/zones/{zone_id}/rrset/");
        let resp = match http::get(&url, &[(hdr, token)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let rrsets = v.get("results").and_then(|r| r.as_array())
            .or_else(|| v.as_array());

        let full_name = format!("{}.", name);
        let quoted_value = format!("\"{}\"", value);
        if let Some(rrsets) = rrsets {
            for rrset in rrsets {
                if rrset.get("name").and_then(|n| n.as_str()) == Some(&full_name)
                    && rrset.get("type").and_then(|t| t.as_str()) == Some("TXT")
                {
                    let records = rrset.get("records").and_then(|r| r.as_array());
                    if let Some(records) = records {
                        let rrset_id = rrset.get("id").and_then(|id| id.as_str()).map(|s| s.to_string());
                        let remaining: Vec<&Value> = records.iter()
                            .filter(|r| r.get("content").and_then(|c| c.as_str()) != Some(&quoted_value))
                            .collect();
                        if let Some(rrset_id) = rrset_id {
                            if remaining.is_empty() {
                                let del_url = format!("{BASE_URL}/v2/zones/{zone_id}/rrset/{rrset_id}");
                                let _ = http::delete(&del_url, &[(hdr, token)]);
                            } else {
                                let ttl = rrset.get("ttl").and_then(|t| t.as_u64()).unwrap_or(60);
                                let patch_body = serde_json::json!({
                                    "ttl": ttl,
                                    "records": remaining,
                                });
                                let patch_url = format!("{BASE_URL}/v2/zones/{zone_id}/rrset/{rrset_id}");
                                let _ = http::patch(&patch_url, &serde_json::to_vec(&patch_body).unwrap(), "application/json", &[(hdr, token)]);
                            }
                            return Ok(());
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

fn record_id(v: &Value) -> Option<String> {
    v.get("id").and_then(|i| {
        if i.is_string() { i.as_str().map(|s| s.to_string()) }
        else if i.is_u64() { i.as_u64().map(|n| n.to_string()) }
        else if i.is_i64() { i.as_i64().map(|n| n.to_string()) }
        else { None }
    })
}
