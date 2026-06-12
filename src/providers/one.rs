use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::json as j;
use crate::providers::{DnsProvider, ProviderResult};

pub struct One {
    username: String,
    password: String,
}

impl DnsProvider for One {
    fn slug() -> &'static str {
        "one"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ONE_Username", "ONE_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("ONE_Username")
            .ok_or_else(|| Error::Config("ONE_Username required".into()))?
            .clone();
        let password = env.get("ONE_Password")
            .ok_or_else(|| Error::Config("ONE_Password required".into()))?
            .clone();
        Ok(Box::new(One { username, password }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let cookie = self.login()?;
        let (main_domain, sub_domain) = self.get_root(name, &cookie)?;
        let url = format!("https://www.one.com/admin/api/domains/{main_domain}/dns/custom_records");
        let body = serde_json::to_vec(&serde_json::json!({
            "type": "dns_custom_records",
            "attributes": {
                "priority": 0,
                "ttl": 600,
                "type": "TXT",
                "prefix": sub_domain,
                "content": value,
            }
        })).unwrap();
        let resp = http::post(&url, &body, "application/json",
            &[("Cookie", &cookie)])
            .map_err(|e| Error::Provider(format!("One.com add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("One.com add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let cookie = match self.login() {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };
        let (main_domain, sub_domain) = match self.get_root(name, &cookie) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let list_url = format!("https://www.one.com/admin/api/domains/{main_domain}/dns/custom_records");
        let resp = match http::get(&list_url, &[("Cookie", &cookie)]) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if let Some(records) = j::get_array(&v, &["result", "data"]) {
            for record in records {
                let attrs = match j::get_object(record, &["attributes"]) {
                    Some(a) => a,
                    None => continue,
                };
                if attrs.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && attrs.get("prefix").and_then(|p| p.as_str()) == Some(sub_domain.as_str())
                    && attrs.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = record.get("id").and_then(|i| i.as_str()) {
                        let del_url = format!("https://www.one.com/admin/api/domains/{main_domain}/dns/custom_records/{id}");
                        let _ = http::delete(&del_url, &[("Cookie", &cookie)]);
                    }
                }
            }
        }
        Ok(())
    }
}

impl One {
    fn login(&self) -> Result<String, Error> {
        let data = format!(
            "loginDomain=true&displayUsername={}&username={}&targetDomain=&password1={}&loginTarget=",
            self.username, self.username, self.password
        );
        let resp = http::post(
            "https://www.one.com/admin/login.do",
            data.as_bytes(),
            "application/x-www-form-urlencoded",
            &[],
        ).map_err(|e| Error::Provider(format!("One.com login: {e}")))?;
        let cookie = resp.headers.iter()
            .find(|(k, v)| k.as_str() == "set-cookie" && v.contains("OneSIDCrmAdmin"))
            .map(|(_, v)| {
                let end = v.find(';').unwrap_or(v.len());
                v[..end].to_string()
            })
            .ok_or_else(|| Error::Provider("One.com login: session cookie not found".into()))?;
        Ok(cookie)
    }

    fn get_root<'a>(&self, domain: &str, cookie: &str) -> Result<(String, String), Error> {
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                continue;
            }
            let url = format!("https://www.one.com/admin/api/domains/{h}/dns/custom_records");
            let resp = http::get(&url, &[("Cookie", cookie)])
                .map_err(|e| Error::Provider(format!("One.com zone: {e}")))?;
            if !resp.body.contains("CRMRST_000302") {
                let sub_domain = parts[..i].join(".");
                return Ok((h, sub_domain));
            }
        }
        Err(Error::Provider(format!("One.com: zone not found for {domain}")))
    }
}
