use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::json;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://ccp.netcup.net/run/webservice/servers/endpoint.php?JSON";

pub struct Netcup {
    customer_number: String,
    api_key: String,
    api_password: String,
}

impl DnsProvider for Netcup {
    fn slug() -> &'static str {
        "netcup"
    }

    fn env_vars() -> &'static [&'static str] {
        &["NETCUP_CUSTOMER_NUMBER", "NETCUP_API_KEY", "NETCUP_API_PASSWORD"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let customer_number = env.get("NETCUP_CUSTOMER_NUMBER")
            .ok_or_else(|| Error::Config("NETCUP_CUSTOMER_NUMBER required".into()))?
            .clone();
        let api_key = env.get("NETCUP_API_KEY")
            .ok_or_else(|| Error::Config("NETCUP_API_KEY required".into()))?
            .clone();
        let api_password = env.get("NETCUP_API_PASSWORD")
            .ok_or_else(|| Error::Config("NETCUP_API_PASSWORD required".into()))?
            .clone();
        Ok(Box::new(Netcup { customer_number, api_key, api_password }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let session = self.login()?;
        let mut records = self.get_dns_records(&session, domain)?;

        let record_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);

        records.push(serde_json::json!({
            "hostname": record_name,
            "type": "TXT",
            "destination": value,
            "deleterecord": "false",
            "state": "yes",
        }));

        self.update_dns_zone(&session, domain, &records)?;
        let _ = self.logout(&session);
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let session = match self.login() {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        let records = match self.get_dns_records(&session, domain) {
            Ok(r) => r,
            Err(_) => { let _ = self.logout(&session); return Ok(()); }
        };

        let record_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);

        let matching: Vec<&Value> = records.iter()
            .filter(|r| {
                r.get("hostname").and_then(|h| h.as_str()) == Some(record_name)
                    && r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && r.get("destination").and_then(|d| d.as_str()) == Some(value)
            })
            .collect();

        if matching.is_empty() {
            let _ = self.logout(&session);
            return Ok(());
        }

        let mut keep: Vec<Value> = records.clone();
        keep.retain(|r| {
            !(r.get("hostname").and_then(|h| h.as_str()) == Some(record_name)
                && r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                && r.get("destination").and_then(|d| d.as_str()) == Some(value))
        });

        let _ = self.update_dns_zone(&session, domain, &keep);
        let _ = self.logout(&session);
        Ok(())
    }
}

impl Netcup {
    fn login(&self) -> Result<String, Error> {
        let body = serde_json::json!({
            "action": "login",
            "param": {
                "customernumber": self.customer_number,
                "apikey": self.api_key,
                "apipassword": self.api_password,
            }
        });
        let resp = http::post(BASE_URL, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("netcup login: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("netcup login response: {e}")))?;

        if json::get_string(&v, &["status"]) != Some("success") {
            let msg = json::get_string(&v, &["shortmessage"])
                .or_else(|| json::get_string(&v, &["longmessage"]))
                .unwrap_or("unknown error");
            return Err(Error::Provider(format!("netcup login failed: {msg}")));
        }

        let session_id = json::get_string_required(&v, &["responsedata", "apisessionid"])?.to_string();
        Ok(session_id)
    }

    fn logout(&self, session: &str) -> Result<(), Error> {
        let body = serde_json::json!({
            "action": "logout",
            "param": {
                "customernumber": self.customer_number,
                "apikey": self.api_key,
                "apisessionid": session,
            }
        });
        let _ = http::post(BASE_URL, &serde_json::to_vec(&body).unwrap(), "application/json", &[]);
        Ok(())
    }

    fn get_dns_records(&self, session: &str, domain: &str) -> Result<Vec<Value>, Error> {
        let body = serde_json::json!({
            "action": "infoDnsZone",
            "param": {
                "domainname": domain,
                "customernumber": self.customer_number,
                "apikey": self.api_key,
                "apisessionid": session,
            }
        });
        let resp = http::post(BASE_URL, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("netcup infoDnsZone: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("netcup infoDnsZone response: {e}")))?;

        if json::get_string(&v, &["status"]) != Some("success") {
            let msg = json::get_string(&v, &["shortmessage"])
                .or_else(|| json::get_string(&v, &["longmessage"]))
                .unwrap_or("unknown error");
            return Err(Error::Provider(format!("netcup infoDnsZone failed: {msg}")));
        }

        let records = json::get_value_required(&v, &["responsedata", "dnsrecords"])?;
        Ok(records.as_array().cloned().unwrap_or_default())
    }

    fn update_dns_zone(&self, session: &str, domain: &str, records: &[Value]) -> Result<(), Error> {
        let body = serde_json::json!({
            "action": "updateDnsZone",
            "param": {
                "domainname": domain,
                "customernumber": self.customer_number,
                "apikey": self.api_key,
                "apisessionid": session,
                "dnsrecordset": {
                    "dnsrecords": records,
                }
            }
        });
        let resp = http::post(BASE_URL, &serde_json::to_vec(&body).unwrap(), "application/json", &[])
            .map_err(|e| Error::Provider(format!("netcup updateDnsZone: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("netcup updateDnsZone: HTTP {} {}", resp.status, resp.body)));
        }

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("netcup updateDnsZone response: {e}")))?;
        if json::get_string(&v, &["status"]) != Some("success") {
            let msg = json::get_string(&v, &["shortmessage"])
                .or_else(|| json::get_string(&v, &["longmessage"]))
                .unwrap_or("unknown error");
            return Err(Error::Provider(format!("netcup updateDnsZone failed: {msg}")));
        }
        Ok(())
    }
}
