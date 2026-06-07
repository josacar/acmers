use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.loopia.se/RPCSERV";

pub struct Loopia {
    user: String,
    pass: String,
}

impl DnsProvider for Loopia {
    fn slug() -> &'static str {
        "loopia"
    }

    fn env_vars() -> &'static [&'static str] {
        &["LOOPIA_User", "LOOPIA_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let user = env.get("LOOPIA_User")
            .ok_or_else(|| Error::Config("LOOPIA_User required".into()))?
            .clone();
        let pass = env.get("LOOPIA_Password")
            .ok_or_else(|| Error::Config("LOOPIA_Password required".into()))?
            .clone();
        Ok(Box::new(Loopia { user, pass }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = self.resolve_zone(domain)?;
        let (record_domain, record_subdomain) = split_domain(&zone, name);
        let record = format!(
            r#"<member><name>type</name><value><string>TXT</string></value></member>
      <member><name>ttl</name><value><int>300</int></value></member>
      <member><name>rdata</name><value><string>{v}</string></value></member>
      <member><name>priority</name><value><int>0</int></value></member>"#,
            v = xml_escape(value)
        );
        let xml = xmlrpc_call("addZoneRecord", &[
            XmlRpcParam::String(self.user.clone()),
            XmlRpcParam::String(self.pass.clone()),
            XmlRpcParam::String(record_domain),
            XmlRpcParam::String(record_subdomain),
            XmlRpcParam::Struct(record),
        ]);
        let resp = http::post(BASE_URL, xml.as_bytes(), "text/xml", &[])
            .map_err(|e| Error::Provider(format!("loopia addZoneRecord: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("loopia addZoneRecord: HTTP {} {}", resp.status, resp.body)));
        }
        check_fault(&resp.body)?;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = match self.resolve_zone(domain) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let (record_domain, record_subdomain) = split_domain(&zone, name);
        let record_id = match self.find_record_id(&zone, &record_domain, &record_subdomain, value) {
            Ok(Some(id)) => id,
            Ok(None) => return Ok(()),
            Err(_) => return Ok(()),
        };
        let xml = xmlrpc_call("removeZoneRecord", &[
            XmlRpcParam::String(self.user.clone()),
            XmlRpcParam::String(self.pass.clone()),
            XmlRpcParam::String(record_domain),
            XmlRpcParam::String(record_subdomain),
            XmlRpcParam::Int(record_id),
        ]);
        let resp = http::post(BASE_URL, xml.as_bytes(), "text/xml", &[])
            .map_err(|e| Error::Provider(format!("loopia removeZoneRecord: {e}")))?;
        if resp.status >= 400 {
            return Ok(());
        }
        let _ = check_fault(&resp.body);
        Ok(())
    }
}

fn split_domain(zone: &str, name: &str) -> (String, String) {
    if name == zone || name.is_empty() {
        return (zone.to_string(), "@".to_string());
    }
    if name.ends_with(zone) {
        let prefix_end = name.len() - zone.len();
        if prefix_end > 1 {
            let sub = &name[..prefix_end - 1];
            return (zone.to_string(), sub.to_string());
        }
        return (zone.to_string(), "@".to_string());
    }
    (name.to_string(), "@".to_string())
}

impl Loopia {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let struct_body = format!(
            r#"<member><name>username</name><value><string>{u}</string></value></member>
      <member><name>password</name><value><string>{p}</string></value></member>"#,
            u = xml_escape(&self.user),
            p = xml_escape(&self.pass)
        );
        let xml = xmlrpc_call("getDomains", &[XmlRpcParam::Struct(struct_body)]);
        let resp = http::post(BASE_URL, xml.as_bytes(), "text/xml", &[])
            .map_err(|e| Error::Provider(format!("loopia getDomains: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("loopia getDomains: HTTP {} {}", resp.status, resp.body)));
        }
        check_fault(&resp.body)?;
        let domains = extract_all_strings_in_array(&resp.body);
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len().saturating_sub(1) {
            let candidate = parts[i..].join(".");
            for d in &domains {
                if d == &candidate {
                    return Ok(d.clone());
                }
            }
        }
        for d in &domains {
            if domain == d || domain.ends_with(&format!(".{d}")) {
                return Ok(d.clone());
            }
        }
        Err(Error::Provider(format!("loopia: zone not found for {domain}")))
    }

    fn find_record_id(&self, _zone: &str, record_domain: &str, record_subdomain: &str, value: &str) -> Result<Option<i64>, Error> {
        let xml = xmlrpc_call("getZoneRecords", &[
            XmlRpcParam::String(self.user.clone()),
            XmlRpcParam::String(self.pass.clone()),
            XmlRpcParam::String(record_domain.to_string()),
            XmlRpcParam::String(record_subdomain.to_string()),
        ]);
        let resp = http::post(BASE_URL, xml.as_bytes(), "text/xml", &[])
            .map_err(|e| Error::Provider(format!("loopia getZoneRecords: {e}")))?;
        if resp.status >= 400 {
            return Ok(None);
        }
        if let Err(_) = check_fault(&resp.body) {
            return Ok(None);
        }
        let members = extract_all_members(&resp.body);
        for member in &members {
            let rec_type = extract_value_in(member, "member", "type");
            let rec_rdata = extract_value_in(member, "member", "rdata");
            if rec_type.as_deref() == Some("TXT") && rec_rdata.as_deref() == Some(value) {
                if let Some(id_str) = extract_value_in(member, "member", "record_id") {
                    if let Ok(id) = id_str.parse::<i64>() {
                        return Ok(Some(id));
                    }
                }
            }
        }
        Ok(None)
    }
}

fn check_fault(xml: &str) -> Result<(), Error> {
    if xml.contains("<name>faultCode</name>") {
        let code = extract_value(xml, "faultCode").unwrap_or_else(|| "unknown".into());
        let msg = extract_value(xml, "faultString").unwrap_or_else(|| "unknown".into());
        return Err(Error::Provider(format!("loopia fault [{code}]: {msg}")));
    }
    Ok(())
}

enum XmlRpcParam {
    String(String),
    Int(i64),
    Struct(String),
}

fn xmlrpc_call(method: &str, params: &[XmlRpcParam]) -> String {
    let mut params_xml = String::new();
    for param in params {
        match param {
            XmlRpcParam::String(s) => {
                params_xml.push_str(&format!(
                    r#"<param><value><string>{}</string></value></param>"#,
                    xml_escape(s)
                ));
            }
            XmlRpcParam::Int(i) => {
                params_xml.push_str(&format!(
                    r#"<param><value><int>{i}</int></value></param>"#
                ));
            }
            XmlRpcParam::Struct(body) => {
                params_xml.push_str(&format!(
                    r#"<param><value><struct>{body}</struct></value></param>"#
                ));
            }
        }
    }
    format!(
        r#"<?xml version="1.0"?>
<methodCall>
  <methodName>{method}</methodName>
  <params>
    {params_xml}
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

fn extract_value(xml: &str, name: &str) -> Option<String> {
    let name_tag = format!("<name>{name}</name>");
    let idx = xml.find(&name_tag)?;
    let rest = &xml[idx + name_tag.len()..];
    let val_start = rest.find("<value>")?;
    let val = &rest[val_start + 7..];
    for tag in &["<string>", "<int>", "<i4>", "<double>", "<boolean>"] {
        if let Some(inner_start) = val.find(tag) {
            let inner = &val[inner_start + tag.len()..];
            let end_tag = tag.replace('<', "</");
            if let Some(inner_end) = inner.find(&end_tag) {
                return Some(inner[..inner_end].to_string());
            }
        }
    }
    if let Some(inner_end) = val.find("</value>") {
        return Some(val[..inner_end].to_string());
    }
    None
}

fn extract_value_in(xml: &str, container: &str, name: &str) -> Option<String> {
    let container_open = format!("<{container}>");
    let container_close = format!("</{container}>");
    let mut pos = 0;
    while let Some(start) = xml[pos..].find(&container_open) {
        let abs_start = pos + start + container_open.len();
        let mut depth = 1;
        let mut search_pos = abs_start;
        let mut end = abs_start;
        while search_pos < xml.len() {
            if let Some(close_idx) = xml[search_pos..].find(&container_close) {
                let next = search_pos + close_idx;
                if let Some(next_open) = xml[search_pos..next].find(&container_open) {
                    depth += 1;
                    search_pos = search_pos + next_open + container_open.len();
                } else {
                    depth -= 1;
                    if depth == 0 {
                        end = next + container_close.len();
                        break;
                    }
                    search_pos = next + container_close.len();
                }
            } else {
                break;
            }
        }
        if depth == 0 {
            let section = &xml[abs_start..end];
            if let Some(v) = extract_value(section, name) {
                return Some(v);
            }
        }
        pos = end.max(abs_start + 1);
    }
    None
}

fn extract_all_members(xml: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut pos = 0;
    while let Some(start) = xml[pos..].find("<member>") {
        let abs_start = pos + start;
        let mut depth = 1;
        let mut search_pos = abs_start + 8;
        while search_pos < xml.len() {
            if let Some(close_idx) = xml[search_pos..].find("</member>") {
                let next = search_pos + close_idx;
                if let Some(next_open) = xml[search_pos..next].find("<member>") {
                    depth += 1;
                    search_pos = search_pos + next_open + 8;
                } else {
                    depth -= 1;
                    if depth == 0 {
                        results.push(xml[abs_start..next + 9].to_string());
                        pos = next + 9;
                        break;
                    }
                    search_pos = next + 9;
                }
            } else {
                break;
            }
        }
        if depth != 0 {
            break;
        }
    }
    results
}

fn extract_all_strings_in_array(xml: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut pos = 0;
    while let Some(start) = xml[pos..].find("<string>") {
        let abs_start = pos + start + 8;
        if let Some(end) = xml[abs_start..].find("</string>") {
            results.push(xml[abs_start..abs_start + end].to_string());
            pos = abs_start + end + 9;
        } else {
            break;
        }
    }
    results
}
