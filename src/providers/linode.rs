use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Linode {
    api_key: String,
}

impl DnsProvider for Linode {
    fn slug() -> &'static str {
        "linode"
    }

    fn env_vars() -> &'static [&'static str] {
        &["LINODE_API_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("LINODE_API_KEY")
            .ok_or_else(|| Error::Config("LINODE_API_KEY required".into()))?
            .clone();
        Ok(Box::new(Linode { api_key }))
    }

    fn add_txt(&self, domain: &str, _name: &str, value: &str) -> ProviderResult {
        let (domain_id, sub_domain) = self.get_root(domain)?;
        let params = format!("&DomainID={domain_id}&Type=TXT&Name={sub_domain}&Target={value}");
        let resp = self.api_get("domain.resource.create", &params)?;
        if !resp.body.contains("RESOURCEID") && !resp.body.contains("ResourceID") {
            return Err(Error::Provider(format!("Linode add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, _name: &str, _value: &str) -> ProviderResult {
        let (domain_id, sub_domain) = match self.get_root(domain) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let resp = match self.api_get("domain.resource.list", &format!("&DomainID={domain_id}")) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if let Some(resource_id) = find_resource_id(&resp.body, &sub_domain) {
            let _ = self.api_get("domain.resource.delete",
                &format!("&DomainID={domain_id}&ResourceID={resource_id}"));
        }
        Ok(())
    }
}

impl Linode {
    fn api_get(&self, action: &str, params: &str) -> Result<http::Response, Error> {
        let url = format!("https://api.linode.com/?api_key={}&api_action={}{}",
            self.api_key, action, params);
        http::get(&url, &[("Accept", "application/json")])
            .map_err(|e| Error::Provider(format!("Linode API: {e}")))
    }

    fn get_root(&self, domain: &str) -> Result<(String, String), Error> {
        let resp = self.api_get("domain.list", "")?;
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                continue;
            }
            let needle = format!("\"DOMAIN\":\"{h}\"");
            if resp.body.contains(&needle) {
                if let Some(id) = extract_domain_id(&resp.body, &h) {
                    let sub_domain = parts[..i].join(".");
                    return Ok((id, sub_domain));
                }
            }
        }
        Err(Error::Provider(format!("Linode: domain not found for {domain}")))
    }
}

fn extract_domain_id(body: &str, domain: &str) -> Option<String> {
    let needle = format!("\"DOMAIN\":\"{domain}\"");
    let pos = body.find(&needle)?;
    let start = if pos > 200 { pos - 200 } else { 0 };
    let chunk = &body[start..pos + needle.len() + 200];
    let id_needle = "\"DOMAINID\":";
    let id_pos = chunk.find(id_needle)?;
    let rest = &chunk[id_pos + id_needle.len()..];
    let end = rest.find(|c: char| !c.is_ascii_digit())?;
    if end == 0 { return None; }
    Some(rest[..end].to_string())
}

fn find_resource_id(body: &str, sub_domain: &str) -> Option<String> {
    let needle = format!("\"NAME\":\"{sub_domain}\"");
    let pos = body.find(&needle)?;
    let start = if pos > 200 { pos - 200 } else { 0 };
    let chunk = &body[start..pos + needle.len() + 200];
    let id_needle = "\"RESOURCEID\":";
    let id_pos = chunk.find(id_needle)?;
    let rest = &chunk[id_pos + id_needle.len()..];
    let end = rest.find(|c: char| !c.is_ascii_digit())?;
    if end == 0 { return None; }
    Some(rest[..end].to_string())
}
