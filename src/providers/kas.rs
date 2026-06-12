use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const RATE_LIMIT_SECS: u64 = 5;

pub struct Kas {
    login: String,
    auth_type: String,
    auth_data: String,
}

impl DnsProvider for Kas {
    fn slug() -> &'static str {
        "kas"
    }

    fn env_vars() -> &'static [&'static str] {
        &["KAS_Login", "KAS_Authtype", "KAS_Authdata"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let login = env.get("KAS_Login")
            .ok_or_else(|| Error::Config("KAS_Login required".into()))?
            .clone();
        let auth_type = env.get("KAS_Authtype")
            .cloned()
            .unwrap_or_else(|| "plain".to_string());
        let auth_data = env.get("KAS_Authdata")
            .ok_or_else(|| Error::Config("KAS_Authdata required".into()))?
            .clone();
        Ok(Box::new(Kas { login, auth_type, auth_data }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let fulldomain = if name.ends_with(domain) {
            name.to_string()
        } else {
            format!("{}.{}", name, domain)
        };

        let api_url = discover_wsdl_endpoint("KasApi")?;
        let auth_url = discover_wsdl_endpoint("KasAuth")?;
        let token = self.get_credential_token(&auth_url)?;
        let (zone, record_name) = self.get_zone_and_record_name(&api_url, &token, &fulldomain)?;

        let record_ids = self.get_record_ids(&api_url, &token, &zone, &record_name, value)?;
        for id in &record_ids {
            self.delete_record(&api_url, &token, id)?;
        }

        self.add_record(&api_url, &token, &zone, &record_name, value)
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let fulldomain = if name.ends_with(domain) {
            name.to_string()
        } else {
            format!("{}.{}", name, domain)
        };

        let api_url = match discover_wsdl_endpoint("KasApi") {
            Ok(u) => u,
            Err(_) => return Ok(()),
        };
        let auth_url = match discover_wsdl_endpoint("KasAuth") {
            Ok(u) => u,
            Err(_) => return Ok(()),
        };
        let token = match self.get_credential_token(&auth_url) {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };
        let (zone, record_name) = match self.get_zone_and_record_name(&api_url, &token, &fulldomain) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let record_ids = match self.get_record_ids(&api_url, &token, &zone, &record_name, value) {
            Ok(ids) => ids,
            Err(_) => return Ok(()),
        };
        for id in &record_ids {
            let _ = self.delete_record(&api_url, &token, id);
        }
        Ok(())
    }
}

impl Kas {
    fn rate_limit() {
        if !http::is_test_mode() {
            thread::sleep(Duration::from_secs(RATE_LIMIT_SECS));
        }
    }

    fn get_credential_token(&self, auth_url: &str) -> Result<String, Error> {
        let params = format!(
            r#"{{"kas_login":"{}","kas_auth_type":"{}","kas_auth_data":"{}","session_lifetime":600,"session_update_lifetime":"Y"}}"#,
            self.login, self.auth_type, self.auth_data
        );

        let xml = format!(
            concat!(
                r#"<?xml version="1.0" encoding="UTF-8"?>"#,
                r#"<SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/""#,
                r#" xmlns:ns1="urn:xmethodsKasApiAuthentication""#,
                r#" xmlns:xsd="http://www.w3.org/2001/XMLSchema""#,
                r#" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#,
                r#" xmlns:SOAP-ENC="http://schemas.xmlsoap.org/soap/encoding/""#,
                r#" SOAP-ENV:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">"#,
                r#"<SOAP-ENV:Body><ns1:KasAuth><Params xsi:type="xsd:string">{}"#,
                r#"</Params></ns1:KasAuth></SOAP-ENV:Body></SOAP-ENV:Envelope>"#
            ),
            params
        );

        Self::rate_limit();

        let resp = http::post(auth_url, xml.as_bytes(), "text/xml", &[
            ("SOAPAction", "urn:xmethodsKasApiAuthentication#KasAuth"),
        ]).map_err(|e| Error::Provider(format!("KAS auth request: {e}")))?;

        if resp.body.is_empty() {
            return Err(Error::Provider("KAS: auth response was empty".into()));
        }

        if resp.body.contains("<SOAP-ENV:Fault>") {
            let fault = extract_fault_string(&resp.body);
            return Err(Error::Provider(format!("KAS auth error: {fault}")));
        }

        extract_token(&resp.body)
            .ok_or_else(|| Error::Provider("KAS: could not extract credential token".into()))
    }

    fn call_api(&self, api_url: &str, token: &str, action: &str, request_params: &str) -> Result<String, Error> {
        let params = if request_params.is_empty() {
            format!(
                r#"{{"kas_login":"{}","kas_auth_type":"session","kas_auth_data":"{}","kas_action":"{}"}}"#,
                self.login, token, action
            )
        } else {
            format!(
                r#"{{"kas_login":"{}","kas_auth_type":"session","kas_auth_data":"{}","kas_action":"{}","KasRequestParams":{{{}}}}}"#,
                self.login, token, action, request_params
            )
        };

        let xml = format!(
            concat!(
                r#"<?xml version="1.0" encoding="UTF-8"?>"#,
                r#"<SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/""#,
                r#" xmlns:ns1="urn:xmethodsKasApi""#,
                r#" xmlns:xsd="http://www.w3.org/2001/XMLSchema""#,
                r#" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#,
                r#" xmlns:SOAP-ENC="http://schemas.xmlsoap.org/soap/encoding/""#,
                r#" SOAP-ENV:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">"#,
                r#"<SOAP-ENV:Body><ns1:KasApi><Params xsi:type="xsd:string">{}"#,
                r#"</Params></ns1:KasApi></SOAP-ENV:Body></SOAP-ENV:Envelope>"#
            ),
            params
        );

        Self::rate_limit();

        let resp = http::post(api_url, xml.as_bytes(), "text/xml", &[
            ("SOAPAction", "urn:xmethodsKasApi#KasApi"),
        ]).map_err(|e| Error::Provider(format!("KAS API request ({action}): {e}")))?;

        if resp.body.is_empty() {
            return Err(Error::Provider(format!("KAS: {action} response was empty")));
        }

        Ok(resp.body)
    }

    fn check_success(response: &str, action: &str) -> ProviderResult {
        if response.contains("<SOAP-ENV:Fault>") {
            let fault = extract_fault_string(response);
            return Err(Error::Provider(format!("KAS {action} error: {fault}")));
        }
        if !response.contains(r#"<item><key xsi:type="xsd:string">ReturnString</key><value xsi:type="xsd:string">TRUE</value></item>"#) {
            return Err(Error::Provider(format!("KAS: {action} returned unexpected response")));
        }
        Ok(())
    }

    fn get_zone_and_record_name(&self, api_url: &str, token: &str, fulldomain: &str) -> Result<(String, String), Error> {
        let response = self.call_api(api_url, token, "get_domains", "")?;

        if response.contains("<SOAP-ENV:Fault>") {
            let fault = extract_fault_string(&response);
            return Err(Error::Provider(format!("KAS get_domains error: {fault}")));
        }

        let domains = extract_domain_names(&response);
        let temp_domain = fulldomain.trim_end_matches('.');

        let mut rootzone = temp_domain.to_string();
        for d in &domains {
            if temp_domain.ends_with(d.as_str()) && temp_domain.len() >= d.len() {
                rootzone = d.clone();
            }
        }

        let zone = format!("{}.", rootzone);
        let record_name = temp_domain
            .strip_suffix(rootzone.as_str())
            .unwrap_or("")
            .trim_end_matches('.')
            .to_string();

        Ok((zone, record_name))
    }

    fn get_record_ids(&self, api_url: &str, token: &str, zone: &str, record_name: &str, value: &str) -> Result<Vec<String>, Error> {
        let request_params = format!(r#""zone_host":"{}""#, zone);
        let response = self.call_api(api_url, token, "get_dns_settings", &request_params)?;

        if response.contains("<SOAP-ENV:Fault>") {
            let fault = extract_fault_string(&response);
            return Err(Error::Provider(format!("KAS get_dns_settings error: {fault}")));
        }

        Ok(extract_record_ids(&response, record_name, value))
    }

    fn add_record(&self, api_url: &str, token: &str, zone: &str, record_name: &str, value: &str) -> ProviderResult {
        let request_params = format!(
            r#""record_name":"{}","record_type":"TXT","record_data":"{}","record_aux":"0","zone_host":"{}""#,
            record_name, value, zone
        );

        let response = self.call_api(api_url, token, "add_dns_settings", &request_params)?;

        if response.contains("<SOAP-ENV:Fault>") {
            let fault = extract_fault_string(&response);
            if fault == "record_already_exists" {
                return Ok(());
            }
            return Err(Error::Provider(format!("KAS add_dns_settings error: {fault}")));
        }

        Self::check_success(&response, "add_dns_settings")
    }

    fn delete_record(&self, api_url: &str, token: &str, record_id: &str) -> ProviderResult {
        let request_params = format!(r#""record_id":"{}""#, record_id);
        let response = self.call_api(api_url, token, "delete_dns_settings", &request_params)?;

        if response.contains("<SOAP-ENV:Fault>") {
            let fault = extract_fault_string(&response);
            if fault == "record_id_not_found" {
                return Ok(());
            }
            return Err(Error::Provider(format!("KAS delete_dns_settings error: {fault}")));
        }

        Self::check_success(&response, "delete_dns_settings")
    }
}

fn discover_wsdl_endpoint(service: &str) -> Result<String, Error> {
    let wsdl_url = format!("https://kasapi.kasserver.com/soap/wsdl/{}.wsdl", service);

    let resp = http::get(&wsdl_url, &[])
        .map_err(|e| Error::Provider(format!("KAS WSDL discovery ({service}): {e}")))?;

    extract_soap_address(&resp.body)
        .ok_or_else(|| Error::Provider(format!("KAS: could not find SOAP address in {service} WSDL")))
}

fn extract_soap_address(wsdl: &str) -> Option<String> {
    let no_spaces: String = wsdl.chars().filter(|c| !c.is_whitespace()).collect();
    let lower = no_spaces.to_lowercase();
    let idx = lower.find("<soap:addresslocation=")?;
    let after = &no_spaces[idx + "<soap:addresslocation=".len()..];
    let quote = after.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let rest = &after[1..];
    let end = rest.find(quote)?;
    let url = &rest[..end];
    if url.starts_with("http") {
        Some(url.to_string())
    } else {
        None
    }
}

fn extract_fault_string(xml: &str) -> String {
    let open = "<faultstring>";
    let close = "</faultstring>";
    let cleaned = xml.replace('\n', "").replace('\r', "");
    if let Some(start) = cleaned.find(open) {
        let content_start = start + open.len();
        if let Some(end) = cleaned[content_start..].find(close) {
            return cleaned[content_start..content_start + end].to_string();
        }
    }
    "unknown error".to_string()
}

fn extract_token(xml: &str) -> Option<String> {
    let cleaned = xml.replace('\n', " ").replace('\r', " ");
    let marker = r#"return xsi:type="xsd:string">"#;
    let idx = cleaned.find(marker)?;
    let content_start = idx + marker.len();
    let end = cleaned[content_start..].find("</return>")?;
    Some(cleaned[content_start..content_start + end].to_string())
}

fn extract_domain_names(xml: &str) -> Vec<String> {
    let mut domains = Vec::new();
    let cleaned = xml.replace('\n', "").replace('\r', "");
    let marker = r#"<key xsi:type="xsd:string">domain_name</key><value xsi:type="xsd:string">"#;
    let mut pos = 0;
    while let Some(start) = cleaned[pos..].find(marker) {
        let content_start = pos + start + marker.len();
        if let Some(end) = cleaned[content_start..].find("</value>") {
            domains.push(cleaned[content_start..content_start + end].to_string());
            pos = content_start + end + 8;
        } else {
            break;
        }
    }
    domains
}

fn extract_record_ids(xml: &str, record_name: &str, value: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let cleaned = xml.replace('\n', "").replace('\r', "");
    let item_marker = r#"<item xsi:type="ns2:Map">"#;
    let mut pos = 0;
    while let Some(start) = cleaned[pos..].find(item_marker) {
        let item_start = pos + start;
        let next_offset = item_start + item_marker.len();
        let item_end = if let Some(ni) = cleaned[next_offset..].find(item_marker) {
            next_offset + ni
        } else {
            cleaned.len()
        };
        let item = &cleaned[item_start..item_end];
        let item_lower = item.to_lowercase();

        let name_match = record_name.is_empty() || item_lower.contains(&record_name.to_lowercase());
        let type_match = item_lower.contains(">txt<");
        let value_match = item.contains(value);

        if name_match && type_match && value_match {
            let id_marker = r#"<item><key xsi:type="xsd:string">record_id</key><value xsi:type="xsd:string">"#;
            if let Some(id_start) = item.find(id_marker) {
                let id_content_start = id_start + id_marker.len();
                if let Some(id_end) = item[id_content_start..].find("</value>") {
                    ids.push(item[id_content_start..id_content_start + id_end].to_string());
                }
            }
        }

        pos = item_end;
    }
    ids
}
