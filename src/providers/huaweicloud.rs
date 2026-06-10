use std::collections::HashMap;

use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const IAM_API: &str = "https://iam.myhuaweicloud.com";
const DNS_API: &str = "https://dns.ap-southeast-1.myhuaweicloud.com";

pub struct Huaweicloud {
    username: String,
    password: String,
    domain_name: String,
}

impl DnsProvider for Huaweicloud {
    fn slug() -> &'static str {
        "huaweicloud"
    }

    fn env_vars() -> &'static [&'static str] {
        &["HUAWEICLOUD_Username", "HUAWEICLOUD_Password", "HUAWEICLOUD_DomainName"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("HUAWEICLOUD_Username")
            .ok_or_else(|| Error::Config("HUAWEICLOUD_Username required".into()))?.clone();
        let password = env.get("HUAWEICLOUD_Password")
            .ok_or_else(|| Error::Config("HUAWEICLOUD_Password required".into()))?.clone();
        let domain_name = env.get("HUAWEICLOUD_DomainName")
            .ok_or_else(|| Error::Config("HUAWEICLOUD_DomainName required".into()))?.clone();
        Ok(Box::new(Huaweicloud { username, password, domain_name }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = self.get_token()?;
        let zone_id = self.get_zone_id(&token, name)?;
        self.add_record(&token, &zone_id, name, value)
    }

    fn remove_txt(&self, _domain: &str, name: &str, _value: &str) -> ProviderResult {
        let token = match self.get_token() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("warning: huaweicloud cleanup login failed: {e}");
                return Ok(());
            }
        };
        let zone_id = match self.get_zone_id(&token, name) {
            Ok(z) => z,
            Err(e) => {
                eprintln!("warning: huaweicloud cleanup zone not found: {e}");
                return Ok(());
            }
        };
        let mut retry = 50;
        loop {
            let record_id = match self.get_recordset_id(&token, name, &zone_id) {
                Ok(Some(id)) => id,
                Ok(None) => return Ok(()),
                Err(e) => {
                    eprintln!("warning: huaweicloud cleanup failed: {e}");
                    return Ok(());
                }
            };
            if retry == 0 {
                eprintln!("warning: huaweicloud failed to remove record after 50 attempts");
                return Ok(());
            }
            retry -= 1;
            if let Err(e) = self.rm_record(&token, &zone_id, &record_id) {
                eprintln!("warning: huaweicloud rm record: {e}");
                return Ok(());
            }
        }
    }
}

impl Huaweicloud {
    fn get_token(&self) -> Result<String, Error> {
        let body = serde_json::json!({
            "auth": {
                "identity": {
                    "methods": ["password"],
                    "password": {
                        "user": {
                            "name": self.username,
                            "password": self.password,
                            "domain": {
                                "name": self.domain_name
                            }
                        }
                    }
                },
                "scope": {
                    "project": {
                        "name": "ap-southeast-1"
                    }
                }
            }
        });
        let resp = http::post(
            &format!("{IAM_API}/v3/auth/tokens"),
            &serde_json::to_vec(&body).unwrap(),
            "application/json;charset=utf8",
            &[],
        ).map_err(|e| Error::Provider(format!("huaweicloud get token: {e}")))?;
        if resp.status >= 300 {
            return Err(Error::Provider(format!("huaweicloud get token: HTTP {} {}", resp.status, resp.body)));
        }
        resp.headers.get("x-subject-token")
            .cloned()
            .ok_or_else(|| Error::Provider("huaweicloud: no X-Subject-Token in response".into()))
    }

    fn get_zone_id(&self, token: &str, fulldomain: &str) -> Result<String, Error> {
        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                break;
            }
            let url = format!("{DNS_API}/v2/zones?name={h}");
            let resp = http::get(&url, &[("X-Auth-Token", token)])
                .map_err(|e| Error::Provider(format!("huaweicloud list zones: {e}")))?;
            let v: Value = serde_json::from_str(&resp.body)
                .map_err(|e| Error::Json(format!("huaweicloud zones: {e}")))?;
            if let Some(zones) = v.get("zones").and_then(|z| z.as_array()) {
                for zone in zones {
                    let zone_name = zone.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let zone_id = zone.get("id").and_then(|n| n.as_str()).unwrap_or("");
                    if zone_name == format!("{h}.") && !zone_id.is_empty() {
                        return Ok(zone_id.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("huaweicloud: zone not found for {fulldomain}")))
    }

    fn get_recordset_id(&self, token: &str, fulldomain: &str, zone_id: &str) -> Result<Option<String>, Error> {
        let url = format!("{DNS_API}/v2/zones/{zone_id}/recordsets?name={fulldomain}&status=ACTIVE");
        let resp = http::get(&url, &[("X-Auth-Token", token)])
            .map_err(|e| Error::Provider(format!("huaweicloud list recordsets: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("huaweicloud recordsets: {e}")))?;
        if let Some(recordsets) = v.get("recordsets").and_then(|r| r.as_array()) {
            for rs in recordsets {
                if let Some(id) = rs.get("id").and_then(|i| i.as_str()) {
                    return Ok(Some(id.to_string()));
                }
            }
        }
        Ok(None)
    }

    fn add_record(&self, token: &str, zone_id: &str, fulldomain: &str, txtvalue: &str) -> ProviderResult {
        let url = format!("{DNS_API}/v2/zones/{zone_id}/recordsets?name={fulldomain}&status=ACTIVE");
        let resp = http::get(&url, &[("X-Auth-Token", token)])
            .map_err(|e| Error::Provider(format!("huaweicloud get records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("huaweicloud records: {e}")))?;

        let existing = v.get("recordsets").and_then(|r| r.as_array())
            .and_then(|arr| arr.first())
            .and_then(|rs| rs.get("records").and_then(|r| r.as_array()).cloned());

        let quoted_value = format!("\"{txtvalue}\"");
        let mut records: Vec<Value> = existing.unwrap_or_default();
        records.push(Value::String(quoted_value));

        let body = serde_json::json!({
            "name": format!("{fulldomain}."),
            "description": "ACME Challenge",
            "type": "TXT",
            "ttl": 1,
            "records": records,
        });

        let record_id = self.get_recordset_id(token, fulldomain, zone_id)?;

        let http_resp = if let Some(rid) = record_id {
            let put_url = format!("{DNS_API}/v2/zones/{zone_id}/recordsets/{rid}");
            http::put(&put_url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("X-Auth-Token", token)])
                .map_err(|e| Error::Provider(format!("huaweicloud update record: {e}")))?
        } else {
            let post_url = format!("{DNS_API}/v2/zones/{zone_id}/recordsets");
            http::post(&post_url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("X-Auth-Token", token)])
                .map_err(|e| Error::Provider(format!("huaweicloud add record: {e}")))?
        };

        if http_resp.status != 202 {
            return Err(Error::Provider(format!("huaweicloud add record: HTTP {} {}", http_resp.status, http_resp.body)));
        }
        Ok(())
    }

    fn rm_record(&self, token: &str, zone_id: &str, record_id: &str) -> ProviderResult {
        let url = format!("{DNS_API}/v2/zones/{zone_id}/recordsets/{record_id}");
        http::delete(&url, &[("X-Auth-Token", token)])
            .map_err(|e| Error::Provider(format!("huaweicloud delete record: {e}")))?;
        Ok(())
    }
}
