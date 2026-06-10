use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const AUTODNS_API: &str = "https://gateway.autodns.com";

pub struct Autodns {
    user: String,
    password: String,
    context: String,
}

impl DnsProvider for Autodns {
    fn slug() -> &'static str {
        "autodns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["AUTODNS_USER", "AUTODNS_PASSWORD", "AUTODNS_CONTEXT"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let user = env.get("AUTODNS_USER")
            .ok_or_else(|| Error::Config("AUTODNS_USER required".into()))?
            .clone();
        let password = env.get("AUTODNS_PASSWORD")
            .ok_or_else(|| Error::Config("AUTODNS_PASSWORD required".into()))?
            .clone();
        let context = env.get("AUTODNS_CONTEXT")
            .ok_or_else(|| Error::Config("AUTODNS_CONTEXT required".into()))?
            .clone();
        Ok(Box::new(Autodns { user, password, context }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone, sub_domain, system_ns) = self.find_zone(name)?;
        self.zone_update(&zone, &sub_domain, value, &system_ns, true)
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone, sub_domain, system_ns) = match self.find_zone(name) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        match self.zone_update(&zone, &sub_domain, value, &system_ns, false) {
            Ok(()) => Ok(()),
            Err(e) => {
                eprintln!("warning: autodns cleanup failed: {e}");
                Ok(())
            }
        }
    }
}

impl Autodns {
    fn build_auth_xml(&self) -> String {
        format!(
            "<auth><user>{}</user><password>{}</password><context>{}</context></auth>",
            xml_escape(&self.user),
            xml_escape(&self.password),
            xml_escape(&self.context),
        )
    }

    fn build_zone_inquire_xml(&self, zone: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<request>
  {}
  <task>
    <code>0205</code>
    <view>
      <children>1</children>
      <limit>1</limit>
    </view>
    <where>
      <key>name</key>
      <operator>eq</operator>
      <value>{}</value>
    </where>
  </task>
</request>"#,
            self.build_auth_xml(),
            xml_escape(zone),
        )
    }

    fn build_zone_update_xml(&self, zone: &str, subdomain: &str, txtvalue: &str, system_ns: &str, add: bool) -> String {
        let tag = if add { "rr_add" } else { "rr_rem" };
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<request>
  {}
  <task>
    <code>0202001</code>
    <default>
      <{tag}>
        <name>{}</name>
        <ttl>600</ttl>
        <type>TXT</type>
        <value>{}</value>
      </{tag}>
    </default>
    <zone>
      <name>{}</name>
      <system_ns>{}</system_ns>
    </zone>
  </task>
</request>"#,
            self.build_auth_xml(),
            xml_escape(subdomain),
            xml_escape(txtvalue),
            xml_escape(zone),
            xml_escape(system_ns),
            tag = tag,
        )
    }

    fn api_call(&self, xml_body: &str) -> Result<String, Error> {
        let resp = http::post(AUTODNS_API, xml_body.as_bytes(), "application/xml", &[])
            .map_err(|e| Error::Provider(format!("autodns api call: {e}")))?;
        if !resp.body.contains("<type>success</type>") {
            return Err(Error::Provider(format!("autodns api error: {}", resp.body)));
        }
        Ok(resp.body)
    }

    fn find_zone(&self, domain: &str) -> Result<(String, String, String), Error> {
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 1..parts.len() {
            let candidate = parts[i..].join(".");
            if candidate.is_empty() {
                break;
            }
            let xml = self.build_zone_inquire_xml(&candidate);
            let response = match self.api_call(&xml) {
                Ok(r) => r,
                Err(_) => continue,
            };
            if response.contains("<summary>1</summary>") {
                let zone = xml_find_text(&response, "name")
                    .ok_or_else(|| Error::Provider("autodns: no zone name in response".into()))?;
                let system_ns = xml_find_text(&response, "system_ns")
                    .ok_or_else(|| Error::Provider("autodns: no system_ns in response".into()))?;
                let sub_domain = parts[..i].join(".");
                return Ok((zone, sub_domain, system_ns));
            }
        }
        Err(Error::Provider(format!("autodns: zone not found for {domain}")))
    }

    fn zone_update(&self, zone: &str, subdomain: &str, txtvalue: &str, system_ns: &str, add: bool) -> Result<(), Error> {
        let xml = self.build_zone_update_xml(zone, subdomain, txtvalue, system_ns, add);
        self.api_call(&xml)?;
        Ok(())
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn xml_find_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    if let Some(start) = xml.find(&open) {
        let content_start = start + open.len();
        if let Some(end) = xml[content_start..].find(&close) {
            return Some(xml[content_start..content_start + end].to_string());
        }
    }
    None
}
