use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Durabledns {
    user: String,
    key: String,
}

const BASE_URL: &str = "https://durabledns.com/services/dns";

impl DnsProvider for Durabledns {
    fn slug() -> &'static str {
        "durabledns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DD_API_User", "DD_API_Key"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let user = env.get("DD_API_User")
            .ok_or_else(|| Error::Config("DD_API_User required".into()))?
            .clone();
        let key = env.get("DD_API_Key")
            .ok_or_else(|| Error::Config("DD_API_Key required".into()))?
            .clone();
        Ok(Box::new(Durabledns { user, key }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (sub_domain, zone) = self.get_root(name)?;
        let zonename = format!("{zone}.");
        let body = self.build_soap_xml("createRecord", &[
            ("string", "zonename", &zonename),
            ("string", "name", &sub_domain),
            ("string", "type", "TXT"),
            ("string", "data", value),
            ("int", "aux", "0"),
            ("int", "ttl", "10"),
            ("string", "ddns_enabled", "N"),
        ]);
        let resp = self.soap_request("createRecord", &body)?;
        if !resp.contains("createRecordResponse") {
            return Err(Error::Provider(format!("durabledns create failed: {resp}")));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (_sub_domain, zone) = self.get_root(name)?;
        let zonename = format!("{zone}.");

        let body = self.build_soap_xml("listRecords", &[
            ("string", "zonename", &zonename),
        ]);
        let resp = self.soap_request("listRecords", &body)?;

        let subtxt = if value.len() > 30 { &value[..30] } else { value };
        let record_id = match Self::extract_record_id(&resp, subtxt) {
            Some(id) => id,
            None => return Ok(()),
        };

        let body = self.build_soap_xml("deleteRecord", &[
            ("string", "zonename", &zonename),
            ("int", "id", &record_id),
        ]);
        let resp = self.soap_request("deleteRecord", &body)?;
        if !resp.contains("Success") && !resp.contains("deleteRecordResponse") {
            eprintln!("warning: durabledns delete may have failed: {resp}");
        }
        Ok(())
    }
}

impl Durabledns {
    fn soap_request(&self, method: &str, xml: &str) -> Result<String, Error> {
        let url = format!("{BASE_URL}/{method}.php");
        let urn = format!("{method}wsdl");
        let action = format!("\"urn:{urn}#{method}\"");
        let resp = http::post(&url, xml.as_bytes(), "text/xml; charset=utf-8", &[
            ("SOAPAction", &action),
        ]).map_err(|e| Error::Provider(format!("durabledns {method}: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("durabledns {method}: {} {}", resp.status, resp.body)));
        }
        Ok(resp.body)
    }

    fn build_soap_xml(&self, method: &str, params: &[(&str, &str, &str)]) -> String {
        let urn = format!("{method}wsdl");
        let mut body_params = format!(
            "<apiuser xsi:type=\"xsd:string\">{}</apiuser>\
             <apikey xsi:type=\"xsd:string\">{}</apikey>",
            self.user, self.key
        );
        for (t, k, v) in params {
            body_params.push_str(&format!("<{k} xsi:type=\"xsd:{t}\">{v}</{k}>"));
        }

        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
             <soap:Envelope xmlns:soap=\"http://schemas.xmlsoap.org/soap/envelope/\" \
             xmlns:soapenc=\"http://schemas.xmlsoap.org/soap/encoding/\" \
             xmlns:tns=\"urn:{urn}\" \
             xmlns:types=\"urn:{urn}/encodedTypes\" \
             xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
             xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\">\
             <soap:Body soap:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
             <tns:{method}>{body_params}</tns:{method}>\
             </soap:Body>\
             </soap:Envelope>"
        )
    }

    fn get_root(&self, fulldomain: &str) -> Result<(String, String), Error> {
        let body = self.build_soap_xml("listZones", &[]);
        let resp = self.soap_request("listZones", &body)?;

        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                continue;
            }
            let origin = format!(">{h}.</origin>");
            if resp.contains(&origin) {
                let sub_domain = if i == 0 {
                    String::new()
                } else {
                    parts[..i].join(".")
                };
                return Ok((sub_domain, h));
            }
        }
        Err(Error::Provider(format!("durabledns zone not found for {fulldomain}")))
    }

    fn extract_record_id(response: &str, subtxt: &str) -> Option<String> {
        let items: Vec<&str> = response.split("<item").collect();
        for item in items {
            if item.contains(subtxt) {
                if let Some(start) = item.find("<id xsi:type=\"xsd:int\">") {
                    let after = &item[start + "<id xsi:type=\"xsd:int\">".len()..];
                    if let Some(end) = after.find("</id>") {
                        return Some(after[..end].to_string());
                    }
                }
            }
        }
        None
    }
}
