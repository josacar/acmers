use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.euserv.net";

pub struct Euserv {
    username: String,
    password: String,
}

impl DnsProvider for Euserv {
    fn slug() -> &'static str {
        "euserv"
    }

    fn env_vars() -> &'static [&'static str] {
        &["EUSERV_Username", "EUSERV_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("EUSERV_Username")
            .ok_or_else(|| Error::Config("EUSERV_Username required".into()))?
            .clone();
        let password = env.get("EUSERV_Password")
            .ok_or_else(|| Error::Config("EUSERV_Password required".into()))?
            .clone();
        Ok(Box::new(Euserv { username, password }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let (domain_id, zone) = self.resolve_zone(domain)?;
        let sub_domain = compute_subdomain(&zone, name);
        let struct_body = format!(
            r#"<member><name>login</name><value><string>{login}</string></value></member>
<member><name>password</name><value><string>{pass}</string></value></member>
<member><name>domain_id</name><value><int>{domain_id}</int></value></member>
<member><name>dns_record_subdomain</name><value><string>{sub}</string></value></member>
<member><name>dns_record_type</name><value><string>TXT</string></value></member>
<member><name>dns_record_value</name><value><string>{val}</string></value></member>
<member><name>dns_record_ttl</name><value><int>300</int></value></member>"#,
            login = xml_escape(&self.username),
            pass = xml_escape(&self.password),
            domain_id = domain_id,
            sub = xml_escape(&sub_domain),
            val = xml_escape(value),
        );
        let xml = xmlrpc_call("domain.dns_create_record", &struct_body);
        let resp = http::post(BASE_URL, xml.as_bytes(), "text/xml", &[])
            .map_err(|e| Error::Provider(format!("euserv dns_create_record: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("euserv dns_create_record: HTTP {} {}", resp.status, resp.body)));
        }
        check_status(&resp.body, "dns_create_record")?;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, _name: &str, value: &str) -> ProviderResult {
        let (domain_id, _zone) = match self.resolve_zone(domain) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let struct_body = format!(
            r#"<member><name>login</name><value><string>{login}</string></value></member>
<member><name>password</name><value><string>{pass}</string></value></member>
<member><name>domain_id</name><value><int>{domain_id}</int></value></member>"#,
            login = xml_escape(&self.username),
            pass = xml_escape(&self.password),
            domain_id = domain_id,
        );
        let xml = xmlrpc_call("domain.dns_get_active_records", &struct_body);
        let resp = match http::post(BASE_URL, xml.as_bytes(), "text/xml", &[]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if resp.status >= 400 {
            return Ok(());
        }
        if !check_status(&resp.body, "dns_get_active_records").is_ok() {
            return Ok(());
        }
        let record_id = match find_record_id_by_value(&resp.body, value) {
            Some(id) => id,
            None => return Ok(()),
        };
        let del_struct = format!(
            r#"<member><name>login</name><value><string>{login}</string></value></member>
<member><name>password</name><value><string>{pass}</string></value></member>
<member><name>dns_record_id</name><value><int>{rid}</int></value></member>"#,
            login = xml_escape(&self.username),
            pass = xml_escape(&self.password),
            rid = record_id,
        );
        let del_xml = xmlrpc_call("domain.dns_delete_record", &del_struct);
        let del_resp = match http::post(BASE_URL, del_xml.as_bytes(), "text/xml", &[]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let _ = check_status(&del_resp.body, "dns_delete_record");
        Ok(())
    }
}

impl Euserv {
    fn resolve_zone(&self, domain: &str) -> Result<(String, String), Error> {
        let struct_body = format!(
            r#"<member><name>login</name><value><string>{login}</string></value></member>
<member><name>password</name><value><string>{pass}</string></value></member>"#,
            login = xml_escape(&self.username),
            pass = xml_escape(&self.password),
        );
        let xml = xmlrpc_call("domain.get_domain_orders", &struct_body);
        let resp = http::post(BASE_URL, xml.as_bytes(), "text/xml", &[])
            .map_err(|e| Error::Provider(format!("euserv get_domain_orders: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("euserv get_domain_orders: HTTP {} {}", resp.status, resp.body)));
        }
        check_status(&resp.body, "get_domain_orders")?;
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len().saturating_sub(1) {
            let candidate = parts[i..].join(".");
            if resp.body.contains(&format!(">{candidate}<")) {
                let domain_id = extract_domain_id(&resp.body, &candidate)
                    .ok_or_else(|| Error::Provider(format!("euserv: could not find domain_id for {candidate}")))?;
                return Ok((domain_id, candidate));
            }
        }
        Err(Error::Provider(format!("euserv: zone not found for {domain}")))
    }
}

fn compute_subdomain(zone: &str, name: &str) -> String {
    if name == zone {
        return "@".to_string();
    }
    if let Some(prefix) = name.strip_suffix(&format!(".{zone}")) {
        if prefix.is_empty() {
            return "@".to_string();
        }
        return prefix.to_string();
    }
    name.to_string()
}

fn check_status(xml: &str, context: &str) -> Result<(), Error> {
    if xml.contains("<member><name>status</name><value><i4>100</i4></value></member>") {
        return Ok(());
    }
    Err(Error::Provider(format!("euserv {context}: status not 100")))
}

fn extract_domain_id(xml: &str, domain: &str) -> Option<String> {
    let needle = format!(">domain_name<");
    let mut pos = 0;
    while let Some(idx) = xml[pos..].find(&needle) {
        let abs_idx = pos + idx;
        let after_name = &xml[abs_idx..];
        if after_name.contains(&format!(">{domain}<")) {
            let search_from = abs_idx;
            if let Some(did_idx) = xml[search_from..].find(">domain_id<") {
                let after_did = &xml[search_from + did_idx..];
                if let Some(i4_start) = after_did.find("<i4>") {
                    let val_start = search_from + did_idx + i4_start + 4;
                    if let Some(i4_end) = xml[val_start..].find("</i4>") {
                        return Some(xml[val_start..val_start + i4_end].to_string());
                    }
                }
            }
        }
        pos = abs_idx + needle.len();
    }
    None
}

fn find_record_id_by_value(xml: &str, value: &str) -> Option<String> {
    let needle = format!(">dns_record_content<");
    let mut pos = 0;
    while let Some(idx) = xml[pos..].find(&needle) {
        let abs_idx = pos + idx;
        let after = &xml[abs_idx..];
        if after.contains(&format!(">{value}<")) {
            let before = &xml[..abs_idx];
            let mut search_pos = 0;
            let mut last_id = None;
            while let Some(name_idx) = before[search_pos..].find("</name><value><struct>") {
                let abs_name = search_pos + name_idx;
                let line = &before[search_pos..abs_name];
                if let Some(name_start) = line.rfind("<name>") {
                    let id_str = &line[name_start + 6..];
                    if id_str.chars().all(|c| c.is_ascii_digit()) && !id_str.is_empty() {
                        last_id = Some(id_str.to_string());
                    }
                }
                search_pos = abs_name + 22;
            }
            return last_id;
        }
        pos = abs_idx + needle.len();
    }
    None
}

fn xmlrpc_call(method: &str, struct_body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<methodCall>
  <methodName>{method}</methodName>
  <params>
    <param>
      <value>
        <struct>
          {struct_body}
        </struct>
      </value>
    </param>
  </params>
</methodCall>"#
    )
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
