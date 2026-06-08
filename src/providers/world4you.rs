use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://my.world4you.com/en";

pub struct World4you {
    username: String,
    password: String,
}

impl DnsProvider for World4you {
    fn slug() -> &'static str {
        "world4you"
    }

    fn env_vars() -> &'static [&'static str] {
        &["WORLD4YOU_Username", "WORLD4YOU_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("WORLD4YOU_Username")
            .ok_or_else(|| Error::Config("WORLD4YOU_Username required".into()))?.clone();
        let password = env.get("WORLD4YOU_Password")
            .ok_or_else(|| Error::Config("WORLD4YOU_Password required".into()))?.clone();
        Ok(Box::new(World4you { username, password }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let sessid = self.login()?;
        let (paketnr, record_name) = self.resolve_zone(name, &sessid)?;
        let url = format!("{BASE_URL}/{paketnr}/dns");
        let cookie = format!("W4YSESSID={sessid}");

        let form_resp = http::get(&url, &[("Cookie", &cookie)])
            .map_err(|e| Error::Provider(format!("world4you get dns form: {e}")))?;

        let form_iddp = extract_form_value(&form_resp.body, "AddDnsRecordForm[uniqueFormIdDP]")
            .ok_or_else(|| Error::Provider("world4you: cannot parse uniqueFormIdDP".into()))?;
        let form_token = extract_form_value(&form_resp.body, "AddDnsRecordForm[_token]")
            .ok_or_else(|| Error::Provider("world4you: cannot parse AddDnsRecordForm[_token]".into()))?;

        let body = format!(
            "AddDnsRecordForm[name]={}&AddDnsRecordForm[dnsType][type]=TXT&AddDnsRecordForm[value]={}&AddDnsRecordForm[uniqueFormIdDP]={}&AddDnsRecordForm[_token]={}",
            url_encode(&record_name), url_encode(value), url_encode(&form_iddp), url_encode(&form_token)
        );
        let resp = http::post(&url, body.as_bytes(), "application/x-www-form-urlencoded", &[("Cookie", &cookie)])
            .map_err(|e| Error::Provider(format!("world4you add TXT post: {e}")))?;

        if resp.status == 302 || resp.body.contains("successfully") {
            let check = http::get(&url, &[("Cookie", &cookie)])
                .map_err(|e| Error::Provider(format!("world4you verify add: {e}")))?;
            if check.body.contains("successfully") {
                return Ok(());
            }
        }
        if resp.body.contains("form-error-message") {
            let msg = extract_error_message(&resp.body).unwrap_or_else(|| "unknown error".into());
            return Err(Error::Provider(format!("world4you add TXT failed: {msg}")));
        }
        Err(Error::Provider("world4you: failed to add TXT record".into()))
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let sessid = match self.login() {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        let (paketnr, _record_name) = match self.resolve_zone(name, &sessid) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let fqdn = name.to_lowercase();
        let url = format!("{BASE_URL}/{paketnr}/dns");
        let cookie = format!("W4YSESSID={sessid}");

        let form_resp = match http::get(&url, &[("Cookie", &cookie)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };

        let form_iddp = match extract_form_value(&form_resp.body, "DeleteDnsRecordForm[uniqueFormIdDP]") {
            Some(v) => v,
            None => return Ok(()),
        };
        let form_token = match extract_form_value(&form_resp.body, "DeleteDnsRecordForm[_token]") {
            Some(v) => v,
            None => return Ok(()),
        };

        let record_id = match find_record_id(&form_resp.body, &fqdn, value) {
            Some(id) => id,
            None => return Ok(()),
        };

        let del_url = format!("{BASE_URL}/{paketnr}/dns/record/delete");
        let body = format!(
            "DeleteDnsRecordForm[id]={}&DeleteDnsRecordForm[uniqueFormIdDP]={}&DeleteDnsRecordForm[_token]={}",
            url_encode(&record_id), url_encode(&form_iddp), url_encode(&form_token)
        );
        let resp = match http::post(&del_url, body.as_bytes(), "application/x-www-form-urlencoded", &[("Cookie", &cookie)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };

        if resp.status == 302 || resp.body.contains("successfully") {
            return Ok(());
        }
        Ok(())
    }
}

impl World4you {
    fn login(&self) -> Result<String, Error> {
        let login_url = format!("{BASE_URL}/login");
        let page_resp = http::get(&login_url, &[])
            .map_err(|e| Error::Provider(format!("world4you login page: {e}")))?;

        let sessid = extract_sessid(&page_resp)
            .ok_or_else(|| Error::Provider("world4you: cannot parse W4YSESSID".into()))?;

        let Some(csrf_token) = extract_input_value(&page_resp.body, "_csrf_token") else {
            return Ok(sessid);
        };

        let cookie = format!("W4YSESSID={sessid}");
        let body = format!(
            "_username={}&_password={}&_csrf_token={}",
            url_encode(&self.username), url_encode(&self.password), url_encode(&csrf_token)
        );
        let resp = http::post(&login_url, body.as_bytes(), "application/x-www-form-urlencoded",
            &[("Cookie", &cookie), ("X-Requested-With", "XMLHttpRequest")])
            .map_err(|e| Error::Provider(format!("world4you login post: {e}")))?;

        if !resp.body.contains("\"success\":true") && !resp.body.contains("\"success\": true") {
            let msg = extract_json_message(&resp.body).unwrap_or_else(|| "unknown error".into());
            return Err(Error::Provider(format!("world4you login failed: {msg}")));
        }

        Ok(extract_sessid(&resp).unwrap_or(sessid))
    }

    fn resolve_zone(&self, fqdn: &str, sessid: &str) -> Result<(String, String), Error> {
        let cookie = format!("W4YSESSID={sessid}");
        let resp = http::get(&format!("{BASE_URL}/"), &[("Cookie", &cookie)])
            .map_err(|e| Error::Provider(format!("world4you dashboard: {e}")))?;

        let fqdn_lower = fqdn.to_lowercase();
        let mut pos = 0;

        while pos < resp.body.len() {
            let Some(rel) = resp.body[pos..].find("\"fqdn\":\"") else { break };
            let abs_fqdn_pos = pos + rel;
            let fqdn_start = abs_fqdn_pos + 8;
            let Some(fqdn_end) = resp.body[fqdn_start..].find('"') else { break };
            let domain = &resp.body[fqdn_start..fqdn_start + fqdn_end];

            if fqdn_lower == domain || fqdn_lower.ends_with(&format!(".{domain}")) {
                let search_start = if abs_fqdn_pos > 500 { abs_fqdn_pos - 500 } else { 0 };
                let context = &resp.body[search_start..abs_fqdn_pos];
                if let Some(id_rel) = context.rfind("\"id\":") {
                    let id_start = search_start + id_rel + 5;
                    let id_str = &resp.body[id_start..];
                    let id_end = id_str.find(|c: char| !c.is_ascii_digit()).unwrap_or(id_str.len());
                    if id_end > 0 {
                        let paketnr = &id_str[..id_end];
                        let record_name = if fqdn_lower == domain {
                            String::new()
                        } else {
                            fqdn_lower[..fqdn_lower.len() - domain.len() - 1].to_string()
                        };
                        return Ok((paketnr.to_string(), record_name));
                    }
                }
            }
            pos = fqdn_start + fqdn_end + 1;
        }

        Err(Error::Provider(format!("world4you: zone not found for {fqdn}")))
    }
}

fn extract_sessid(resp: &http::Response) -> Option<String> {
    let set_cookie = resp.headers.get("set-cookie")?;
    let pos = set_cookie.find("W4YSESSID=")?;
    let start = pos + 10;
    let rest = &set_cookie[start..];
    let end = rest.find(';').unwrap_or(rest.len());
    let sessid = &rest[..end];
    if sessid.is_empty() { None } else { Some(sessid.to_string()) }
}

fn extract_input_value(html: &str, name: &str) -> Option<String> {
    let pattern = format!("name=\"{name}\"");
    let pos = html.find(&pattern)?;
    let search_start = if pos > 300 { pos - 300 } else { 0 };
    let search_end = (pos + pattern.len() + 300).min(html.len());
    let context = &html[search_start..search_end];
    let val_pos = context.find("value=\"")?;
    let val_start = val_pos + 7;
    let val_end = context[val_start..].find('"')?;
    Some(context[val_start..val_start + val_end].to_string())
}

fn extract_form_value(html: &str, field_name: &str) -> Option<String> {
    let pattern = format!("name=\"{field_name}\"");
    let pos = html.find(&pattern)?;
    let search_start = if pos > 300 { pos - 300 } else { 0 };
    let search_end = (pos + pattern.len() + 300).min(html.len());
    let context = &html[search_start..search_end];
    let val_pos = context.find("value=\"")?;
    let val_start = val_pos + 7;
    let val_end = context[val_start..].find('"')?;
    Some(context[val_start..val_start + val_end].to_string())
}

fn find_record_id(html: &str, fqdn: &str, value: &str) -> Option<String> {
    let pos = html.find("data-records=\"")?;
    let start = pos + 14;
    let end = html[start..].find('"')?;
    let raw = &html[start..start + end];
    let decoded = raw.replace("&quot;", "\"");

    let mut search = decoded.as_str();
    loop {
        let obj_start = search.find('{')?;
        let obj_end = search[obj_start..].find('}')?;
        let record = &search[obj_start..obj_start + obj_end + 1];

        if record.contains(&format!("\"type\":\"TXT\""))
            && record.contains(&format!("\"name\":\"{fqdn}\""))
            && record.contains(&format!("\"value\":\"{value}\""))
        {
            if let Some(id_pos) = record.find("\"id\":\"") {
                let id_start = id_pos + 6;
                if let Some(id_end) = record[id_start..].find('"') {
                    return Some(record[id_start..id_start + id_end].to_string());
                }
            }
        }

        let next = obj_start + obj_end + 1;
        if next >= search.len() { break; }
        search = &search[next..];
    }
    None
}

fn extract_json_message(body: &str) -> Option<String> {
    let pos = body.find("\"message\":\"")?;
    let start = pos + 11;
    let end = body[start..].find('"')?;
    Some(body[start..start + end].to_string())
}

fn extract_error_message(html: &str) -> Option<String> {
    let pos = html.find("form-error-message")?;
    let after = &html[pos..];
    let tag_start = after.find('>')? + 1;
    let tag_end = after[tag_start..].find('<')?;
    let msg = after[tag_start..tag_start + tag_end].trim();
    if msg.is_empty() { None } else { Some(msg.to_string()) }
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push(hex((b >> 4) & 0xf));
                out.push(hex(b & 0xf));
            }
        }
    }
    out
}

fn hex(n: u8) -> char {
    if n < 10 { (b'0' + n) as char } else { (b'A' + n - 10) as char }
}
