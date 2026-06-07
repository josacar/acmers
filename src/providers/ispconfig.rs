use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Ispconfig {
    base_url: String,
    user: String,
    pass: String,
}

impl DnsProvider for Ispconfig {
    fn slug() -> &'static str {
        "ispconfig"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ISPC_User", "ISPC_Password", "ISPC_Api", "ISPC_Api_Insecure"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let user = env.get("ISPC_User")
            .ok_or_else(|| Error::Config("ISPC_User required".into()))?
            .clone();
        let pass = env.get("ISPC_Password")
            .ok_or_else(|| Error::Config("ISPC_Password required".into()))?
            .clone();
        let base_url = env.get("ISPC_Api")
            .ok_or_else(|| Error::Config("ISPC_Api required".into()))?
            .clone();
        Ok(Box::new(Ispconfig { base_url, user, pass }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let session = self.login()?;
        let zone_id = self.get_zone_id(&session, domain)?;
        let record_name = extract_record_name(name, domain);
        let _record_id = self.create_txt(&session, &zone_id, &record_name, value)?;
        let _ = self.logout(&session);
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let session = match self.login() {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        let zone_id = match self.get_zone_id(&session, domain) {
            Ok(z) => z,
            Err(_) => { let _ = self.logout(&session); return Ok(()); }
        };
        let record_name = extract_record_name(name, domain);
        let records = match self.get_zone_records(&session, &zone_id) {
            Ok(r) => r,
            Err(_) => { let _ = self.logout(&session); return Ok(()); }
        };
        for rec in &records {
            let rec_name = rec.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let rec_value = rec.get("value").and_then(|v| v.as_str()).unwrap_or("");
            if rec_name == record_name && rec_value == value {
                if let Some(rec_id) = rec.get("id").and_then(|v| v.as_i64()) {
                    let _ = self.delete_txt(&session, rec_id);
                }
            }
        }
        let _ = self.logout(&session);
        Ok(())
    }
}

fn extract_record_name(name: &str, domain: &str) -> String {
    if name == domain || name.is_empty() {
        return domain.to_string();
    }
    if name.ends_with(domain) {
        return name.to_string();
    }
    format!("{name}.{domain}")
}

impl Ispconfig {
    fn login(&self) -> Result<String, Error> {
        let body = serde_json::json!({
            "username": self.user,
            "password": self.pass,
            "client_login": false
        });
        let resp = http::post(&self.base_url, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("ispconfig login: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("ispconfig login: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("ispconfig login response: {e}")))?;
        if v.get("code").and_then(|c| c.as_str()) != Some("ok") {
            return Err(Error::Provider(format!("ispconfig login failed: {}", resp.body)));
        }
        v.get("response").and_then(|r| r.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| Error::Provider("ispconfig login: no session_id in response".into()))
    }

    fn logout(&self, _session: &str) -> Result<(), Error> {
        Ok(())
    }

    fn get_zone_id(&self, session: &str, domain: &str) -> Result<String, Error> {
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len().saturating_sub(1) {
            let candidate = parts[i..].join(".");
            let req_body = serde_json::json!({
                "session_id": session,
                "dns_zone_get_by_user": {"origin": candidate}
            });
            let resp = http::post(&self.base_url, &serde_json::to_vec(&req_body).unwrap(), "application/json", &[])
                .map_err(|e| Error::Provider(format!("ispconfig zone lookup: {e}")))?;
            if resp.status >= 400 {
                continue;
            }
            let v: Value = serde_json::from_str(&resp.body)
                .map_err(|e| Error::Json(format!("ispconfig zone response: {e}")))?;
            if v.get("code").and_then(|c| c.as_str()) != Some("ok") {
                continue;
            }
            if let Some(response) = v.get("response") {
                if let Some(arr) = response.as_array() {
                    if let Some(first) = arr.first() {
                        if let Some(id) = first.get("id") {
                            if let Some(id_int) = id.as_i64() {
                                return Ok(id_int.to_string());
                            }
                            if let Some(id_str) = id.as_str() {
                                return Ok(id_str.to_string());
                            }
                        }
                    }
                }
                if let Some(obj) = response.as_object() {
                    if let Some(id) = obj.get("id") {
                        if let Some(id_int) = id.as_i64() {
                            return Ok(id_int.to_string());
                        }
                        if let Some(id_str) = id.as_str() {
                            return Ok(id_str.to_string());
                        }
                    }
                }
                if let Some(s) = response.as_str() {
                    if s.chars().all(|c| c.is_ascii_digit()) {
                        return Ok(s.to_string());
                    }
                    if let Ok(inner) = serde_json::from_str::<Value>(s) {
                        if let Some(arr) = inner.as_array() {
                            if let Some(first) = arr.first() {
                                if let Some(id) = first.get("id") {
                                    if let Some(id_int) = id.as_i64() {
                                        return Ok(id_int.to_string());
                                    }
                                    if let Some(id_str) = id.as_str() {
                                        return Ok(id_str.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("ispconfig: zone not found for {domain}")))
    }

    fn create_txt(&self, session: &str, zone_id: &str, name: &str, value: &str) -> Result<String, Error> {
        let zone_id_int: i64 = zone_id.parse().unwrap_or(0);
        let req_body = serde_json::json!({
            "session_id": session,
            "dns_txt_add": {
                "server_id": 1,
                "zone": zone_id_int,
                "name": name,
                "value": value,
                "ttl": 120
            }
        });
        let resp = http::post(&self.base_url, &serde_json::to_vec(&req_body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("ispconfig dns_txt_add: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("ispconfig dns_txt_add: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("ispconfig dns_txt_add response: {e}")))?;
        if v.get("code").and_then(|c| c.as_str()) != Some("ok") {
            return Err(Error::Provider(format!("ispconfig dns_txt_add failed: {}", resp.body)));
        }
        if let Some(response) = v.get("response") {
            if let Some(s) = response.as_str() {
                return Ok(s.to_string());
            }
            if let Some(n) = response.as_i64() {
                return Ok(n.to_string());
            }
        }
        Ok("unknown".to_string())
    }

    fn delete_txt(&self, session: &str, record_id: i64) -> Result<(), Error> {
        let req_body = serde_json::json!({
            "session_id": session,
            "dns_txt_delete": {
                "primary_id": record_id
            }
        });
        let resp = http::post(&self.base_url, &serde_json::to_vec(&req_body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("ispconfig dns_txt_delete: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("ispconfig dns_txt_delete: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn get_zone_records(&self, session: &str, zone_id: &str) -> Result<Vec<Value>, Error> {
        let zone_id_int: i64 = zone_id.parse().unwrap_or(0);
        let req_body = serde_json::json!({
            "session_id": session,
            "dns_zone_get": {"primary_id": zone_id_int}
        });
        let resp = http::post(&self.base_url, &serde_json::to_vec(&req_body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("ispconfig dns_zone_get: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("ispconfig dns_zone_get: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("ispconfig dns_zone_get response: {e}")))?;
        if v.get("code").and_then(|c| c.as_str()) != Some("ok") {
            return Err(Error::Provider(format!("ispconfig dns_zone_get failed: {}", resp.body)));
        }
        let response = v.get("response");
        let records = find_records(response);
        Ok(records)
    }
}

fn find_records(v: Option<&Value>) -> Vec<Value> {
    let val = match v {
        Some(v) => v,
        None => return Vec::new(),
    };
    for field in &["items", "records", "dns_records"] {
        if let Some(arr) = val.get(field).and_then(|a| a.as_array()) {
            return arr.clone();
        }
    }
    if let Some(obj) = val.as_object() {
        for (_, v) in obj.iter() {
            if let Some(arr) = v.as_array() {
                if !arr.is_empty() && arr.first().and_then(|r| r.get("name")).is_some() {
                    return arr.clone();
                }
            }
        }
    }
    if let Some(s) = val.as_str() {
        if let Ok(inner) = serde_json::from_str::<Value>(s) {
            if let Some(arr) = inner.as_array() {
                if !arr.is_empty() {
                    return arr.clone();
                }
            }
            if let Some(obj) = inner.as_object() {
                for field in &["items", "records", "dns_records"] {
                    if let Some(arr) = obj.get(*field).and_then(|a| a.as_array()) {
                        return arr.clone();
                    }
                }
            }
        }
    }
    Vec::new()
}
