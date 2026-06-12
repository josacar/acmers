use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Openprovider {
    username: String,
    password_hash: String,
}

impl DnsProvider for Openprovider {
    fn slug() -> &'static str {
        "openprovider"
    }

    fn env_vars() -> &'static [&'static str] {
        &["OPENPROVIDER_USER", "OPENPROVIDER_PASSWORD_HASH"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("OPENPROVIDER_USER")
            .ok_or_else(|| Error::Config("OPENPROVIDER_USER required".into()))?
            .clone();
        let password_hash = env.get("OPENPROVIDER_PASSWORD_HASH")
            .ok_or_else(|| Error::Config("OPENPROVIDER_PASSWORD_HASH required".into()))?
            .clone();
        Ok(Box::new(Openprovider { username, password_hash }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (domain_name, domain_extension) = self.find_domain(name)?;
        let existing = self.get_records(&domain_name, &domain_extension)?;
        let sub = name.strip_suffix(&format!(".{domain_name}.{domain_extension}"))
            .or_else(|| name.strip_suffix(&format!("{domain_name}.{domain_extension}")))
            .unwrap_or(name);
        let new_record = format!(
            "<item><name>{sub}</name><type>TXT</type><value>{value}</value><ttl>600</ttl></item>"
        );
        let records_xml = format!("{existing}{new_record}");
        self.modify_zone(&domain_name, &domain_extension, &records_xml)?;
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, _value: &str) -> ProviderResult {
        let (domain_name, domain_extension) = match self.find_domain(name) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let existing = match self.get_records_filtered(&domain_name, &domain_extension, name) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        match self.modify_zone(&domain_name, &domain_extension, &existing) {
            Ok(()) => {}
            Err(e) => eprintln!("warning: OpenProvider remove TXT: {e}"),
        }
        Ok(())
    }
}

impl Openprovider {
    fn request(&self, inner_xml: &str) -> Result<String, Error> {
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><openXML><credentials><username>{}</username><hash>{}</hash></credentials>{}</openXML>",
            self.username, self.password_hash, inner_xml
        );
        let resp = http::post("https://api.openprovider.eu/", xml.as_bytes(), "application/xml", &[])
            .map_err(|e| Error::Provider(format!("OpenProvider: {e}")))?;
        if !resp.body.contains("<code>0</code>") {
            return Err(Error::Provider(format!("OpenProvider API error: {}", resp.body)));
        }
        Ok(resp.body)
    }

    fn find_domain(&self, domain: &str) -> Result<(String, String), Error> {
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 1..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                continue;
            }
            let name_part = h.split('.').next().unwrap_or(&h);
            let xml = format!(
                "<searchDomainRequest><domainNamePattern>{name_part}</domainNamePattern><offset>0</offset></searchDomainRequest>"
            );
            let resp = self.request(&xml)?;
            if let Some((name, ext)) = find_matching_domain(&resp, &h) {
                return Ok((name, ext));
            }
        }
        Err(Error::Provider(format!("OpenProvider: domain not found for {domain}")))
    }

    fn get_records(&self, domain_name: &str, domain_extension: &str) -> Result<String, Error> {
        let full = format!("{domain_name}.{domain_extension}");
        let xml = format!(
            "<searchZoneRecordDnsRequest><name>{full}</name><offset>0</offset></searchZoneRecordDnsRequest>"
        );
        let resp = self.request(&xml)?;
        Ok(extract_all_record_items(&resp, &full))
    }

    fn get_records_filtered(&self, domain_name: &str, domain_extension: &str, exclude_name: &str) -> Result<String, Error> {
        let full = format!("{domain_name}.{domain_extension}");
        let xml = format!(
            "<searchZoneRecordDnsRequest><name>{full}</name><offset>0</offset></searchZoneRecordDnsRequest>"
        );
        let resp = self.request(&xml)?;
        Ok(extract_all_record_items_excluding(&resp, &full, exclude_name))
    }

    fn modify_zone(&self, domain_name: &str, domain_extension: &str, records_xml: &str) -> Result<(), Error> {
        let xml = format!(
            "<modifyZoneDnsRequest><domain><name>{domain_name}</name><extension>{domain_extension}</extension></domain><type>master</type><records><array>{records_xml}</array></records></modifyZoneDnsRequest>"
        );
        self.request(&xml)?;
        Ok(())
    }
}

fn extract_xml_value(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)?;
    let rest = &xml[start + open.len()..];
    let end = rest.find(&close)?;
    Some(rest[..end].to_string())
}

fn find_matching_domain(xml: &str, target: &str) -> Option<(String, String)> {
    let mut pos = 0;
    while let Some(start) = xml[pos..].find("<domain>") {
        let abs_start = pos + start;
        if let Some(end) = xml[abs_start..].find("</domain>") {
            let chunk = &xml[abs_start..abs_start + end + 9];
            let name = extract_xml_value(chunk, "name")?;
            let ext = extract_xml_value(chunk, "extension")?;
            if format!("{name}.{ext}") == target {
                return Some((name, ext));
            }
            pos = abs_start + end + 9;
        } else {
            break;
        }
    }
    None
}

fn extract_all_record_items(xml: &str, full_domain: &str) -> String {
    let allowed_types = ["A", "AAAA", "CNAME", "MX", "SPF", "SRV", "TXT", "TLSA", "SSHFP", "CAA"];
    let mut result = String::new();
    let mut pos = 0;
    while let Some(start) = xml[pos..].find("<item>") {
        let abs_start = pos + start;
        if let Some(end) = xml[abs_start..].find("</item>") {
            let item = &xml[abs_start..abs_start + end + 7];
            if let Some(item_type) = extract_xml_value(item, "type") {
                if allowed_types.contains(&item_type.as_str()) {
                    let simplified = simplify_record_item(item, full_domain);
                    if !simplified.is_empty() {
                        result.push_str(&simplified);
                    }
                }
            }
            pos = abs_start + end + 7;
        } else {
            break;
        }
    }
    result
}

fn extract_all_record_items_excluding(xml: &str, full_domain: &str, exclude_name: &str) -> String {
    let allowed_types = ["A", "AAAA", "CNAME", "MX", "SPF", "SRV", "TXT", "TLSA", "SSHFP", "CAA"];
    let mut result = String::new();
    let mut pos = 0;
    while let Some(start) = xml[pos..].find("<item>") {
        let abs_start = pos + start;
        if let Some(end) = xml[abs_start..].find("</item>") {
            let item = &xml[abs_start..abs_start + end + 7];
            if let Some(item_type) = extract_xml_value(item, "type") {
                if allowed_types.contains(&item_type.as_str()) {
                    let name = extract_xml_value(item, "name").unwrap_or_default();
                    if name.contains(exclude_name) {
                        pos = abs_start + end + 7;
                        continue;
                    }
                    let simplified = simplify_record_item(item, full_domain);
                    if !simplified.is_empty() {
                        result.push_str(&simplified);
                    }
                }
            }
            pos = abs_start + end + 7;
        } else {
            break;
        }
    }
    result
}

fn simplify_record_item(item: &str, full_domain: &str) -> String {
    let name = extract_xml_value(item, "name").unwrap_or_default();
    let rtype = extract_xml_value(item, "type").unwrap_or_default();
    let value = extract_xml_value(item, "value").unwrap_or_default();
    let prio = extract_xml_value(item, "prio").unwrap_or("0".into());
    let ttl = extract_xml_value(item, "ttl").unwrap_or("86400".into());

    let short_name = name
        .strip_suffix(&format!(".{full_domain}"))
        .unwrap_or(if name == full_domain { "" } else { &name });

    format!("<item><name>{short_name}</name><type>{rtype}</type><value>{value}</value><prio>{prio}</prio><ttl>{ttl}</ttl></item>")
}
