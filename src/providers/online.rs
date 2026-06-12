use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const API_BASE: &str = "https://api.online.net/api/v1";

pub struct Online {
    api_key: String,
}

impl DnsProvider for Online {
    fn slug() -> &'static str {
        "online"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ONLINE_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("ONLINE_API_KEY")
            .ok_or_else(|| Error::Config("ONLINE_API_KEY required".into()))?
            .clone();
        Ok(Box::new(Online { api_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone, _sub_domain, real_version) = self.get_root(domain)?;
        let temp_version = self.create_temp_version(&zone)?;
        self.enable_zone(&zone, &temp_version)?;
        self.create_txt_record(&zone, &real_version, name, value)?;
        self.enable_zone(&zone, &real_version)?;
        self.destroy_zone(&zone, &temp_version)?;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, _name: &str, value: &str) -> ProviderResult {
        let (zone, sub_domain, real_version) = match self.get_root(domain) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let record_id = match self.find_record_id(&zone, &real_version, &sub_domain, value) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let temp_version = match self.create_temp_version(&zone) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let _ = self.enable_zone(&zone, &temp_version);
        let _ = self.api_delete(&format!("domain/{zone}/version/{real_version}/zone/{record_id}"));
        let _ = self.enable_zone(&zone, &real_version);
        let _ = self.destroy_zone(&zone, &temp_version);
        Ok(())
    }
}

impl Online {
    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    fn api_get(&self, path: &str) -> Result<http::Response, Error> {
        let url = format!("{API_BASE}/{path}");
        let auth = self.auth_header();
        http::get(&url, &[("Authorization", &auth), ("X-Pretty-JSON", "1")])
            .map_err(|e| Error::Provider(format!("Online.net GET: {e}")))
    }

    fn api_post(&self, path: &str, body: &str) -> Result<http::Response, Error> {
        let url = format!("{API_BASE}/{path}");
        let auth = self.auth_header();
        http::post(&url, body.as_bytes(), "application/x-www-form-urlencoded",
            &[("Authorization", &auth), ("X-Pretty-JSON", "1")])
            .map_err(|e| Error::Provider(format!("Online.net POST: {e}")))
    }

    fn api_patch(&self, path: &str) -> Result<http::Response, Error> {
        let url = format!("{API_BASE}/{path}");
        let auth = self.auth_header();
        http::patch(&url, &[], "application/x-www-form-urlencoded",
            &[("Authorization", &auth), ("X-Pretty-JSON", "1")])
            .map_err(|e| Error::Provider(format!("Online.net PATCH: {e}")))
    }

    fn api_delete(&self, path: &str) -> Result<http::Response, Error> {
        let url = format!("{API_BASE}/{path}");
        let auth = self.auth_header();
        http::delete(&url, &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("Online.net DELETE: {e}")))
    }

    fn get_root(&self, domain: &str) -> Result<(String, String, String), Error> {
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 1..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                continue;
            }
            let resp = self.api_get(&format!("domain/{h}/version/active"))?;
            if !resp.body.contains("Domain not found") && !resp.body.contains("invalid_grant") {
                let sub_domain = parts[..i].join(".");
                let version = extract_uuid_ref(&resp.body)
                    .ok_or_else(|| Error::Provider("Online.net: no uuid_ref in response".into()))?;
                return Ok((h, sub_domain, version));
            }
        }
        Err(Error::Provider(format!("Online.net: zone not found for {domain}")))
    }

    fn create_temp_version(&self, zone: &str) -> Result<String, Error> {
        let resp = self.api_post(&format!("domain/{zone}/version"), "name=acmers")?;
        let version = extract_uuid_ref(&resp.body)
            .ok_or_else(|| Error::Provider("Online.net: no uuid_ref for temp version".into()))?;
        self.create_txt_record(zone, &version, "dummy.acmers", "dummy")?;
        Ok(version)
    }

    fn enable_zone(&self, zone: &str, version: &str) -> Result<(), Error> {
        self.api_patch(&format!("domain/{zone}/version/{version}/enable"))?;
        Ok(())
    }

    fn destroy_zone(&self, zone: &str, version: &str) -> Result<(), Error> {
        self.api_delete(&format!("domain/{zone}/version/{version}"))?;
        Ok(())
    }

    fn create_txt_record(&self, zone: &str, version: &str, name: &str, value: &str) -> Result<(), Error> {
        let data = format!("type=TXT&name={name}&data=%22{value}%22&ttl=60&priority=0");
        self.api_post(&format!("domain/{zone}/version/{version}/zone"), &data)?;
        Ok(())
    }

    fn find_record_id(&self, zone: &str, _version: &str, sub_domain: &str, value: &str) -> Result<String, Error> {
        let resp = self.api_get(&format!("domain/{zone}/version/active"))?;
        let quoted = format!("\\u0022{value}\\u0022");
        let needle = format!("\"name\":\"{sub_domain}\",\"data\":\"{quoted}\"");
        if let Some(pos) = resp.body.find(&needle) {
            let start = if pos > 100 { pos - 100 } else { 0 };
            let chunk = &resp.body[start..pos];
            if let Some(id_pos) = chunk.rfind("\"id\":") {
                let rest = &chunk[id_pos + 5..];
                let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
                if end > 0 {
                    return Ok(rest[..end].to_string());
                }
            }
        }
        Err(Error::Provider("Online.net: record not found".into()))
    }
}

fn extract_uuid_ref(body: &str) -> Option<String> {
    let needle = "\"uuid_ref\":";
    let pos = body.find(needle)?;
    let rest = &body[pos + needle.len()..];
    let quote_start = rest.find('"')?;
    let rest = &rest[quote_start + 1..];
    let quote_end = rest.find('"')?;
    Some(rest[..quote_end].to_string())
}
