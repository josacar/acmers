use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const API_URL: &str = "https://soap.subreg.cz/cmd.php";

pub struct Subreg {
    username: String,
    password: String,
}

impl DnsProvider for Subreg {
    fn slug() -> &'static str { "subreg" }
    fn env_vars() -> &'static [&'static str] { &["SUBREG_API_USERNAME", "SUBREG_API_PASSWORD"] }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("SUBREG_API_USERNAME")
            .ok_or_else(|| Error::Config("SUBREG_API_USERNAME required".into()))?.clone();
        let password = env.get("SUBREG_API_PASSWORD")
            .ok_or_else(|| Error::Config("SUBREG_API_PASSWORD required".into()))?.clone();
        Ok(Box::new(Subreg { username, password }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let ssid = self.login()?;
        let (sub_domain, zone) = self.get_root(&ssid, name)?;
        let inner = format!(
            "<domain>{zone}</domain><record><name>{sub}</name><type>TXT</type><content>{val}</content><prio>0</prio><ttl>120</ttl></record>",
            zone = zone, sub = sub_domain, val = value,
        );
        let resp = self.soap_auth(&ssid, "Add_DNS_Record", &inner)?;
        if !soap_is_ok(&resp) {
            return Err(Error::Provider(format!("subreg Add_DNS_Record: {resp}")));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let ssid = match self.login() {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        let (_sub_domain, zone) = match self.get_root(&ssid, name) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let inner = format!("<domain>{}</domain>", zone);
        let resp = match self.soap_auth(&ssid, "Get_DNS_Zone", &inner) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let record_id = match find_record_id(&resp, &_sub_domain, value) {
            Some(id) => id,
            None => return Ok(()),
        };
        let del_inner = format!("<domain>{}</domain><record><id>{}</id></record>", zone, record_id);
        let _ = self.soap_auth(&ssid, "Delete_DNS_Record", &del_inner);
        Ok(())
    }
}

impl Subreg {
    fn login(&self) -> Result<String, Error> {
        let inner = format!("<login>{}</login><password>{}</password>", self.username, self.password);
        let resp = self.soap_raw("Login", &inner)?;
        if !soap_is_ok(&resp) {
            return Err(Error::Provider(format!("subreg login: {resp}")));
        }
        soap_map_get(&resp, "ssid").ok_or_else(|| Error::Provider("subreg login: no ssid".into()))
    }

    fn get_root(&self, ssid: &str, fulldomain: &str) -> Result<(String, String), Error> {
        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() { continue; }
            let inner = format!("<domain>{}</domain>", h);
            if let Ok(resp) = self.soap_auth(ssid, "Get_DNS_Zone", &inner) {
                if soap_is_ok(&resp) {
                    let sub = if i == 0 { String::new() } else { parts[..i].join(".") };
                    return Ok((sub, h));
                }
            }
        }
        Err(Error::Provider(format!("subreg: zone not found for {fulldomain}")))
    }

    fn soap_auth(&self, ssid: &str, cmd: &str, inner: &str) -> Result<String, Error> {
        let with_ssid = format!("<ssid>{}</ssid>{}", ssid, inner);
        self.soap_raw(cmd, &with_ssid)
    }

    fn soap_raw(&self, cmd: &str, inner: &str) -> Result<String, Error> {
        let body = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
             <SOAP-ENV:Envelope xmlns:SOAP-ENV=\"http://schemas.xmlsoap.org/soap/envelope/\" \
             xmlns:ns1=\"http://soap.subreg.cz/soap\" \
             xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\" \
             xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
             xmlns:SOAP-ENC=\"http://schemas.xmlsoap.org/soap/encoding/\" \
             SOAP-ENV:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
             <SOAP-ENV:Body>\
             <ns1:{cmd}><data>{inner}</data></ns1:{cmd}>\
             </SOAP-ENV:Body>\
             </SOAP-ENV:Envelope>"
        );
        let action = format!("http://soap.subreg.cz/soap#{cmd}");
        let resp = http::post(API_URL, body.as_bytes(), "text/xml", &[
            ("SOAPAction", &action),
        ]).map_err(|e| Error::Provider(format!("subreg {cmd}: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("subreg {cmd}: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(resp.body)
    }
}

fn soap_is_ok(xml: &str) -> bool {
    soap_map_get(xml, "status").as_deref() == Some("ok")
}

fn soap_map_get(xml: &str, key: &str) -> Option<String> {
    let pattern = format!(">{key}</key><value");
    let idx = xml.find(&pattern)?;
    let after = &xml[idx + pattern.len()..];
    let gt = after.find('>')?;
    let content = &after[gt + 1..];
    let end = content.find("</value>")?;
    Some(content[..end].to_string())
}

fn find_record_id(xml: &str, name: &str, value: &str) -> Option<String> {
    let name_key = ">name</key><value";
    let mut pos = 0;
    while let Some(idx) = xml[pos..].find(name_key) {
        let abs = pos + idx;
        let after_tag = &xml[abs + name_key.len()..];
        if let Some(gt) = after_tag.find('>') {
            let content = &after_tag[gt + 1..];
            if let Some(end) = content.find("</value>") {
                if &content[..end] == name {
                    let before = &xml[..abs];
                    let record_id = find_last_id(before);
                    let after = &content[end..];
                    let window = if after.len() > 1000 { &after[..1000] } else { after };
                    if soap_key_eq(window, "type", "TXT") && soap_key_eq(window, "content", value) {
                        return record_id;
                    }
                }
            }
        }
        pos = abs + name_key.len();
    }
    None
}

fn find_last_id(xml: &str) -> Option<String> {
    let id_key = ">id</key><value";
    let mut last = None;
    let mut pos = 0;
    while let Some(idx) = xml[pos..].find(id_key) {
        let abs = pos + idx;
        let after = &xml[abs + id_key.len()..];
        if let Some(gt) = after.find('>') {
            let content = &after[gt + 1..];
            if let Some(end) = content.find("</value>") {
                last = Some(content[..end].to_string());
            }
        }
        pos = abs + id_key.len();
    }
    last
}

fn soap_key_eq(xml: &str, key: &str, expected: &str) -> bool {
    soap_map_get(xml, key).as_deref() == Some(expected)
}
