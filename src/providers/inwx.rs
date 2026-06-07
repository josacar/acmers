use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.domrobot.com/xmlrpc/";

pub struct Inwx {
    user: String,
    pass: String,
    shared_secret: Option<String>,
}

impl DnsProvider for Inwx {
    fn slug() -> &'static str {
        "inwx"
    }

    fn env_vars() -> &'static [&'static str] {
        &["INWX_User", "INWX_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let user = env.get("INWX_User")
            .ok_or_else(|| Error::Config("INWX_User required".into()))?
            .clone();
        let pass = env.get("INWX_Password")
            .ok_or_else(|| Error::Config("INWX_Password required".into()))?
            .clone();
        let shared_secret = env.get("INWX_Shared_Secret").cloned();
        Ok(Box::new(Inwx { user, pass, shared_secret }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let session = self.login()?;
        let zone_id = self.resolve_zone(&session, domain)?;
        let record_name = extract_record_name(name, domain);
        let record_id = self.create_record(&session, &zone_id, &record_name, value)?;
        let _ = self.logout(&session);
        let _ = record_id;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let session = match self.login() {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        let zone_id = match self.resolve_zone(&session, domain) {
            Ok(z) => z,
            Err(_) => { let _ = self.logout(&session); return Ok(()); }
        };
        let record_name = extract_record_name(name, domain);
        let record_id = match self.find_record_id(&session, &zone_id, &record_name, value) {
            Ok(Some(id)) => id,
            Ok(None) => { let _ = self.logout(&session); return Ok(()); }
            Err(_) => { let _ = self.logout(&session); return Ok(()); }
        };
        let _ = self.delete_record(&session, &record_id);
        let _ = self.logout(&session);
        Ok(())
    }
}

fn extract_record_name(name: &str, domain: &str) -> String {
    if name == domain || name.is_empty() {
        String::new()
    } else if name.ends_with(domain) {
        let prefix_end = name.len() - domain.len();
        if prefix_end > 0 {
            name[..prefix_end - 1].to_string()
        } else {
            String::new()
        }
    } else {
        name.to_string()
    }
}

impl Inwx {
    fn login(&self) -> Result<String, Error> {
        let members = if let Some(ref secret) = self.shared_secret {
            format!(
                r#"<member><name>user</name><value><string>{u}</string></value></member>
      <member><name>pass</name><value><string>{p}</string></value></member>
      <member><name>sharedSecret</name><value><string>{s}</string></value></member>"#,
                u = xml_escape(&self.user),
                p = xml_escape(&self.pass),
                s = xml_escape(secret)
            )
        } else {
            format!(
                r#"<member><name>user</name><value><string>{u}</string></value></member>
      <member><name>pass</name><value><string>{p}</string></value></member>"#,
                u = xml_escape(&self.user),
                p = xml_escape(&self.pass)
            )
        };
        let body = xmlrpc_request("account.login", &vec![("struct_body", members.as_str())]);
        let resp = http::post(BASE_URL, body.as_bytes(), "text/xml", &[])
            .map_err(|e| Error::Provider(format!("inwx login: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("inwx login: HTTP {} {}", resp.status, resp.body)));
        }
        let code = xml_extract_value(&resp.body, "code");
        if code.as_deref() != Some("1000") {
            if code.as_deref() == Some("1000") {
            } else if code.as_deref() == Some("2000") {
                return Err(Error::Provider("inwx login: invalid credentials".into()));
            }
            let msg = xml_extract_value(&resp.body, "msg").unwrap_or_else(|| "unknown error".into());
            return Err(Error::Provider(format!("inwx login failed (code={code:?}): {msg}")));
        }
        let tfa = xml_extract_value(&resp.body, "tfa");
        if tfa.as_deref() == Some("GOOGLE-AUTHENTICATOR") {
            if self.shared_secret.is_some() {
            } else {
                return Err(Error::Provider("inwx: 2FA required but INWX_Shared_Secret not set".into()));
            }
            self.login()
        } else {
            let res_data = xml_extract_nested(&resp.body, "resData");
            Ok(res_data.unwrap_or_default())
        }
    }

    fn logout(&self, _session: &str) -> Result<(), Error> {
        Ok(())
    }

    fn resolve_zone(&self, _session: &str, domain: &str) -> Result<String, Error> {
        let struct_body = format!(
            r#"<member><name>domain</name><value><string>{d}</string></value></member>"#,
            d = xml_escape(domain)
        );
        let body = xmlrpc_request("nameserver.list", &[("struct_body", &struct_body)]);
        let resp = http::post(BASE_URL, body.as_bytes(), "text/xml", &[])
            .map_err(|e| Error::Provider(format!("inwx nameserver.list: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("inwx nameserver.list: HTTP {} {}", resp.status, resp.body)));
        }
        let code = xml_extract_value(&resp.body, "code");
        if code.as_deref() != Some("1000") {
            let msg = xml_extract_value(&resp.body, "msg").unwrap_or_else(|| "unknown".into());
            return Err(Error::Provider(format!("inwx nameserver.list failed: {msg}")));
        }
        let res_data = xml_extract_nested(&resp.body, "resData");
        if let Some(data) = res_data {
            let ro_id = xml_extract_value_in(&data, "member", "roId");
            if let Some(id) = ro_id {
                return Ok(id);
            }
            if let Some(id) = xml_extract_value_in(&data, "member", "id") {
                return Ok(id);
            }
        }
        Err(Error::Provider(format!("inwx: zone not found for {domain}")))
    }

    fn find_record_id(&self, _session: &str, zone_id: &str, record_name: &str, value: &str) -> Result<Option<String>, Error> {
        let struct_body = format!(
            r#"<member><name>roId</name><value><int>{zid}</int></value></member>
      <member><name>type</name><value><string>TXT</string></value></member>
      <member><name>name</name><value><string>{rn}</string></value></member>"#,
            zid = zone_id,
            rn = xml_escape(record_name)
        );
        let body = xmlrpc_request("nameserver.info", &[("struct_body", &struct_body)]);
        let resp = http::post(BASE_URL, body.as_bytes(), "text/xml", &[])
            .map_err(|e| Error::Provider(format!("inwx nameserver.info: {e}")))?;
        if resp.status >= 400 {
            return Ok(None);
        }
        let code = xml_extract_value(&resp.body, "code");
        if code.as_deref() != Some("1000") {
            return Ok(None);
        }
        let res_data = match xml_extract_nested(&resp.body, "resData") {
            Some(d) => d,
            None => return Ok(None),
        };
        match xml_extract_value_in(&res_data, "member", "id") {
            Some(id) => Ok(Some(id)),
            None => {
                let records = xml_extract_all_members(&res_data);
                for rec in &records {
                    let rec_type = xml_extract_value_in(rec, "member", "type");
                    let rec_name = xml_extract_value_in(rec, "member", "name");
                    let rec_content = xml_extract_value_in(rec, "member", "content");
                    if rec_type.as_deref() == Some("TXT")
                        && rec_name.as_deref() == Some(record_name)
                        && rec_content.as_deref() == Some(value)
                    {
                        if let Some(id) = xml_extract_value_in(rec, "member", "id") {
                            return Ok(Some(id));
                        }
                    }
                }
                Ok(None)
            }
        }
    }

    fn create_record(&self, _session: &str, zone_id: &str, record_name: &str, value: &str) -> Result<String, Error> {
        let struct_body = format!(
            r#"<member><name>roId</name><value><int>{zid}</int></value></member>
      <member><name>type</name><value><string>TXT</string></value></member>
      <member><name>name</name><value><string>{rn}</string></value></member>
      <member><name>content</name><value><string>{v}</string></value></member>
      <member><name>ttl</name><value><int>300</int></value></member>"#,
            zid = zone_id,
            rn = xml_escape(record_name),
            v = xml_escape(value)
        );
        let body = xmlrpc_request("nameserver.createRecord", &[("struct_body", &struct_body)]);
        let resp = http::post(BASE_URL, body.as_bytes(), "text/xml", &[])
            .map_err(|e| Error::Provider(format!("inwx nameserver.createRecord: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("inwx createRecord: HTTP {} {}", resp.status, resp.body)));
        }
        let code = xml_extract_value(&resp.body, "code");
        if code.as_deref() != Some("1000") {
            let msg = xml_extract_value(&resp.body, "msg").unwrap_or_else(|| "unknown".into());
            return Err(Error::Provider(format!("inwx createRecord failed (code={code:?}): {msg}")));
        }
        let res_data = xml_extract_nested(&resp.body, "resData").unwrap_or_default();
        let record_id = xml_extract_value_in(&res_data, "member", "id").unwrap_or_default();
        if record_id.is_empty() {
            return Err(Error::Provider("inwx createRecord: could not extract record id".into()));
        }
        Ok(record_id)
    }

    fn delete_record(&self, _session: &str, record_id: &str) -> Result<(), Error> {
        let struct_body = format!(
            r#"<member><name>id</name><value><int>{rid}</int></value></member>"#,
            rid = record_id
        );
        let body = xmlrpc_request("nameserver.deleteRecord", &[("struct_body", &struct_body)]);
        let resp = http::post(BASE_URL, body.as_bytes(), "text/xml", &[])
            .map_err(|e| Error::Provider(format!("inwx nameserver.deleteRecord: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("inwx deleteRecord: HTTP {} {}", resp.status, resp.body)));
        }
        let code = xml_extract_value(&resp.body, "code");
        if code.as_deref() != Some("1000") {
            let msg = xml_extract_value(&resp.body, "msg").unwrap_or_else(|| "unknown".into());
            return Err(Error::Provider(format!("inwx deleteRecord failed: {msg}")));
        }
        Ok(())
    }
}

fn xmlrpc_request(method: &str, params: &[(&str, &str)]) -> String {
    let mut params_xml = String::new();
    for (key, value) in params {
        if *key == "struct_body" {
            params_xml.push_str(&format!(
                r#"<param><value><struct>{v}</struct></value></param>"#,
                v = value
            ));
        } else {
            params_xml.push_str(&format!(
                r#"<param><value><string>{v}</string></value></param>"#,
                v = value
            ));
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

fn xml_extract_value(xml: &str, name: &str) -> Option<String> {
    let mut pos = 0;
    let name_tag = format!("<name>{name}</name>");
    while let Some(idx) = xml[pos..].find(&name_tag) {
        let abs_idx = pos + idx + name_tag.len();
        let rest = &xml[abs_idx..];
        if let Some(val_start) = rest.find("<value>") {
            let val = rest[val_start + 7..].to_string();
            for tag in &["<string>", "<int>", "<i4>", "<double>", "<boolean>"] {
                if let Some(inner_start) = val.find(tag) {
                    let inner = val[inner_start + tag.len()..].to_string();
                    let end_tag = tag.replace('<', "</");
                    if let Some(inner_end) = inner.find(&end_tag) {
                        return Some(inner[..inner_end].to_string());
                    }
                }
            }
            if let Some(inner_end) = val.find("</value>") {
                return Some(val[..inner_end].to_string());
            }
        }
        pos = abs_idx;
    }
    None
}

fn xml_extract_value_in(xml: &str, container: &str, name: &str) -> Option<String> {
    let container_open = format!("<{container}>");
    let container_close = format!("</{container}>");
    let mut pos = 0;
    while let Some(start) = xml[pos..].find(&container_open) {
        let abs_start = pos + start + container_open.len();
        let depth_start = abs_start;
        let mut depth = 1;
        let mut end = depth_start;
        let mut search_pos = depth_start;
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
            if let Some(v) = xml_extract_value(section, name) {
                return Some(v);
            }
        }
        pos = end.max(abs_start + 1);
    }
    None
}

fn xml_extract_nested(xml: &str, name: &str) -> Option<String> {
    let open = format!("<name>{name}</name>");
    if let Some(idx) = xml.find(&open) {
        let after_name = &xml[idx + open.len()..];
        if let Some(val_start) = after_name.find("<value>") {
            let val_content = &after_name[val_start + 7..];
            let close_val = val_content.find("</value>")?;
            let inner = &val_content[..close_val];
            if inner.starts_with("<struct>") {
                if let Some(s_end) = inner.find("</struct>") {
                    return Some(inner[..s_end + 9].to_string());
                }
            } else if inner.starts_with("<array>") {
                if let Some(a_end) = inner.find("</array>") {
                    return Some(inner[..a_end + 8].to_string());
                }
            } else if inner.starts_with("<string>") {
                if let Some(s_end) = inner.find("</string>") {
                    return Some(inner[8..s_end].to_string());
                }
            } else if inner.starts_with("<int>") {
                if let Some(i_end) = inner.find("</int>") {
                    return Some(inner[5..i_end].to_string());
                }
            }
            return Some(inner.to_string());
        }
    }
    None
}

fn xml_extract_all_members(xml: &str) -> Vec<String> {
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
                        let member_xml = &xml[abs_start..next + 9];
                        results.push(member_xml.to_string());
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
