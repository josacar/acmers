use std::collections::HashMap;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const API_BASE: &str = "https://api.nic.ru";

pub struct Nic {
    client_id: String,
    client_secret: String,
    username: String,
    password: String,
}

impl DnsProvider for Nic {
    fn slug() -> &'static str {
        "nic"
    }

    fn env_vars() -> &'static [&'static str] {
        &["NIC_ClientID", "NIC_ClientSecret", "NIC_Username", "NIC_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Nic {
            client_id: env.get("NIC_ClientID")
                .ok_or_else(|| Error::Config("NIC_ClientID required".into()))?
                .clone(),
            client_secret: env.get("NIC_ClientSecret")
                .ok_or_else(|| Error::Config("NIC_ClientSecret required".into()))?
                .clone(),
            username: env.get("NIC_Username")
                .ok_or_else(|| Error::Config("NIC_Username required".into()))?
                .clone(),
            password: env.get("NIC_Password")
                .ok_or_else(|| Error::Config("NIC_Password required".into()))?
                .clone(),
        }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = self.get_auth_token()?;
        let (sub_domain, zone, service) = self.get_root(domain, &token)?;
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" ?><request><rr-list><rr><name>{}</name><type>TXT</type><txt><string>{}</string></txt></rr></rr-list></request>"#,
            xml_escape(&sub_domain),
            xml_escape(value),
        );
        let url = format!("{API_BASE}/dns-master/services/{service}/zones/{zone}/records");
        let resp = http::put(&url, xml.as_bytes(), "application/xml", &[
            ("Authorization", &format!("Bearer {token}")),
        ]).map_err(|e| Error::Provider(format!("nic add record: {e}")))?;
        nic_check_response(&resp.body)?;

        let commit_url = format!("{API_BASE}/dns-master/services/{service}/zones/{zone}/commit");
        let resp = http::post(&commit_url, b"", "application/xml", &[
            ("Authorization", &format!("Bearer {token}")),
        ]).map_err(|e| Error::Provider(format!("nic commit: {e}")))?;
        nic_check_response(&resp.body)?;
        let _ = name;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = match self.get_auth_token() {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };
        let (sub_domain, zone, service) = match self.get_root(domain, &token) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };

        let list_url = format!("{API_BASE}/dns-master/services/{service}/zones/{zone}/records");
        let resp = match http::get(&list_url, &[
            ("Authorization", &format!("Bearer {token}")),
        ]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };

        let record_id = match find_record_id(&resp.body, &sub_domain, value) {
            Some(id) => id,
            None => return Ok(()),
        };

        let del_url = format!("{API_BASE}/dns-master/services/{service}/zones/{zone}/records/{record_id}");
        let _ = http::delete_with_body(&del_url, b"", "application/xml", &[
            ("Authorization", &format!("Bearer {token}")),
        ]);

        let commit_url = format!("{API_BASE}/dns-master/services/{service}/zones/{zone}/commit");
        let _ = http::post(&commit_url, b"", "application/xml", &[
            ("Authorization", &format!("Bearer {token}")),
        ]);
        let _ = name;
        Ok(())
    }
}

impl Nic {
    fn get_auth_token(&self) -> Result<String, Error> {
        let creds = base64::encode_std(format!("{}:{}", self.client_id, self.client_secret).as_bytes());
        let body = format!(
            "grant_type=password&username={}&password={}&scope=%28GET%7CPUT%7CPOST%7CDELETE%29%3A%2Fdns-master%2F.%2B",
            self.username, self.password,
        );
        let url = format!("{API_BASE}/oauth/token");
        let resp = http::post(&url, body.as_bytes(), "application/x-www-form-urlencoded", &[
            ("Authorization", &format!("Basic {creds}")),
        ]).map_err(|e| Error::Provider(format!("nic auth: {e}")))?;

        extract_access_token(&resp.body)
            .ok_or_else(|| Error::Provider(format!("nic auth: no access_token in response: {}", resp.body)))
    }

    fn get_root(&self, domain: &str, token: &str) -> Result<(String, String, String), Error> {
        let url = format!("{API_BASE}/dns-master/zones");
        let resp = http::get(&url, &[
            ("Authorization", &format!("Bearer {token}")),
        ]).map_err(|e| Error::Provider(format!("nic list zones: {e}")))?;

        let zones = parse_zones(&resp.body);
        let parts: Vec<&str> = domain.split('.').collect();

        for i in 0..parts.len() {
            let candidate = parts[i..].join(".");
            for z in &zones {
                if z.idn_name == candidate {
                    let sub_domain = if i == 0 {
                        String::new()
                    } else {
                        parts[..i].join(".")
                    };
                    return Ok((sub_domain, z.idn_name.clone(), z.service.clone()));
                }
            }
        }

        Err(Error::Provider(format!("nic: zone not found for {domain}")))
    }
}

struct ZoneInfo {
    idn_name: String,
    service: String,
}

fn parse_zones(xml: &str) -> Vec<ZoneInfo> {
    let mut zones = Vec::new();
    let mut pos = 0;
    while let Some(start) = xml[pos..].find("<zone ") {
        let tag_start = pos + start;
        let tag_end = match xml[tag_start..].find('>') {
            Some(e) => tag_start + e + 1,
            None => break,
        };
        let tag = &xml[tag_start..tag_end];

        let idn_name = extract_attr(tag, "idn-name");
        let service = extract_attr(tag, "service");

        if let (Some(name), Some(svc)) = (idn_name, service) {
            zones.push(ZoneInfo { idn_name: name, service: svc });
        }
        pos = tag_end;
    }
    zones
}

fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let pattern = format!("{attr}=\"");
    if let Some(start) = tag.find(&pattern) {
        let val_start = start + pattern.len();
        if let Some(end) = tag[val_start..].find('"') {
            return Some(tag[val_start..val_start + end].to_string());
        }
    }
    None
}

fn find_record_id(xml: &str, sub_domain: &str, value: &str) -> Option<String> {
    let mut pos = 0;
    while let Some(start) = xml[pos..].find("<rr ") {
        let rr_start = pos + start;
        let rr_end = match xml[rr_start..].find("</rr>") {
            Some(e) => rr_start + e + 5,
            None => break,
        };
        let record = &xml[rr_start..rr_end];

        let id = extract_attr(record, "id");
        let name_tag = find_xml_text(record, "name");
        let string_tag = find_xml_text(record, "string");

        if name_tag.as_deref() == Some(sub_domain) && string_tag.as_deref() == Some(value) {
            if let Some(rid) = id {
                return Some(rid);
            }
        }
        pos = rr_end;
    }
    None
}

fn find_xml_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    if let Some(start) = xml.find(&open) {
        let content_start = start + open.len();
        if let Some(end) = xml[content_start..].find(&close) {
            return Some(xml[content_start..content_start + end].to_string());
        }
    }
    let open_q = format!("<{tag} ");
    if let Some(start) = xml.find(&open_q) {
        let after_tag = start + open_q.len();
        if let Some(gt) = xml[after_tag..].find('>') {
            let content_start = after_tag + gt + 1;
            if let Some(end) = xml[content_start..].find(&close) {
                return Some(xml[content_start..content_start + end].to_string());
            }
        }
    }
    None
}

fn extract_access_token(json: &str) -> Option<String> {
    let needle = "\"access_token\":\"";
    if let Some(start) = json.find(needle) {
        let val_start = start + needle.len();
        if let Some(end) = json[val_start..].find('"') {
            return Some(json[val_start..val_start + end].to_string());
        }
    }
    let needle2 = "\"access_token\": \"";
    if let Some(start) = json.find(needle2) {
        let val_start = start + needle2.len();
        if let Some(end) = json[val_start..].find('"') {
            return Some(json[val_start..val_start + end].to_string());
        }
    }
    None
}

fn nic_check_response(body: &str) -> Result<(), Error> {
    if body.contains("<errors>") {
        let error = find_xml_text(body, "error").unwrap_or_else(|| "unknown error".to_string());
        return Err(Error::Provider(format!("nic API error: {error}")));
    }
    if !body.contains("<status>success</status>") {
        return Err(Error::Provider(format!("nic API: unexpected response: {body}")));
    }
    Ok(())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_access_token() {
        let json = r#"{"token_type":"bearer","access_token":"abc123def","expires_in":3600}"#;
        assert_eq!(extract_access_token(json), Some("abc123def".to_string()));
    }

    #[test]
    fn test_extract_access_token_with_space() {
        let json = r#"{"token_type":"bearer", "access_token": "xyz789"}"#;
        assert_eq!(extract_access_token(json), Some("xyz789".to_string()));
    }

    #[test]
    fn test_extract_access_token_missing() {
        let json = r#"{"error":"invalid_grant"}"#;
        assert_eq!(extract_access_token(json), None);
    }

    #[test]
    fn test_parse_zones() {
        let xml = r#"<response><status>success</status><zones><zone id="1" service="SERV1" name="example.com" idn-name="example.com"/></zones></response>"#;
        let zones = parse_zones(xml);
        assert_eq!(zones.len(), 1);
        assert_eq!(zones[0].idn_name, "example.com");
        assert_eq!(zones[0].service, "SERV1");
    }

    #[test]
    fn test_parse_zones_multiple() {
        let xml = r#"<response><zones>
            <zone id="1" service="S1" name="a.com" idn-name="a.com"/>
            <zone id="2" service="S2" name="b.com" idn-name="b.com"/>
        </zones></response>"#;
        let zones = parse_zones(xml);
        assert_eq!(zones.len(), 2);
        assert_eq!(zones[1].idn_name, "b.com");
        assert_eq!(zones[1].service, "S2");
    }

    #[test]
    fn test_find_record_id() {
        let xml = r#"<response><rr-list>
            <rr id="rec123"><name>_acme-challenge</name><type>TXT</type><txt><string>testval</string></txt></rr>
            <rr id="rec456"><name>other</name><type>TXT</type><txt><string>otherval</string></txt></rr>
        </rr-list></response>"#;
        assert_eq!(find_record_id(xml, "_acme-challenge", "testval"), Some("rec123".to_string()));
        assert_eq!(find_record_id(xml, "other", "otherval"), Some("rec456".to_string()));
        assert_eq!(find_record_id(xml, "nonexistent", "testval"), None);
    }

    #[test]
    fn test_nic_check_response_success() {
        let body = r#"<response><status>success</status></response>"#;
        assert!(nic_check_response(body).is_ok());
    }

    #[test]
    fn test_nic_check_response_error() {
        let body = r#"<response><errors><error code="ERR">Something went wrong</error></errors></response>"#;
        assert!(nic_check_response(body).is_err());
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("a<b>c&d"), "a&lt;b&gt;c&amp;d");
    }
}
