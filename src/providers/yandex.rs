use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::json as j;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Yandex {
    token: String,
}

impl DnsProvider for Yandex {
    fn slug() -> &'static str {
        "yandex"
    }

    fn env_vars() -> &'static [&'static str] {
        &["YANDEX_Token"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let token = env.get("YANDEX_Token")
            .ok_or_else(|| Error::Config("YANDEX_Token required".into()))?
            .clone();
        Ok(Box::new(Yandex { token }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let root_domain = self.find_root(name)?;
        let subdomain = name.strip_suffix(&format!(".{root_domain}")).unwrap_or(name);
        let data = format!("domain={root_domain}&type=TXT&subdomain={subdomain}&content={value}&ttl=60");
        let auth = format!("Token {}", self.token);
        let resp = http::post(
            "https://pddimpex2.yandex.net/add",
            data.as_bytes(),
            "application/x-www-form-urlencoded",
            &[("Authorization", &auth)],
        ).map_err(|e| Error::Provider(format!("Yandex PDD add: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("Yandex PDD add: HTTP {} {}", resp.status, resp.body)));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Yandex PDD: {e}")))?;
        let success = v.get("success").and_then(|s| s.as_str()) == Some("ok");
        if !success {
            return Err(Error::Provider(format!("Yandex PDD add: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let root_domain = match self.find_root(name) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };
        let record_id = match self.find_record(&root_domain, name, value) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let data = format!("domain={root_domain}&record_id={record_id}");
        let auth = format!("Token {}", self.token);
        let _ = http::post(
            "https://pddimpex2.yandex.net/del",
            data.as_bytes(),
            "application/x-www-form-urlencoded",
            &[("Authorization", &auth)],
        );
        Ok(())
    }
}

impl Yandex {
    fn find_root(&self, domain: &str) -> Result<String, Error> {
        let auth = format!("Token {}", self.token);
        let resp = http::get("https://pddimpex2.yandex.net/get_domain_list",
            &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("Yandex PDD list: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Yandex PDD: {e}")))?;
        if let Some(domains) = j::get_array(&v, &["domains"]) {
            for d in domains {
                if let Some(name) = d.get("name").and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        return Ok(name.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("Yandex PDD: domain not found for {domain}")))
    }

    fn find_record(&self, root_domain: &str, name: &str, value: &str) -> Result<String, Error> {
        let auth = format!("Token {}", self.token);
        let url = format!("https://pddimpex2.yandex.net/list?domain={root_domain}");
        let resp = http::get(&url, &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("Yandex PDD list records: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Yandex PDD records: {e}")))?;
        if let Some(records) = j::get_array(&v, &["records"]) {
            let subdomain = name.strip_suffix(&format!(".{root_domain}")).unwrap_or(name);
            for r in records {
                if r.get("type").and_then(|t| t.as_str()) == Some("TXT")
                    && r.get("subdomain").and_then(|s| s.as_str()) == Some(subdomain)
                    && r.get("content").and_then(|c| c.as_str()) == Some(value)
                {
                    if let Some(id) = r.get("record_id").and_then(|i| {
                        if let Some(n) = i.as_i64() { Some(n.to_string()) } else { i.as_str().map(|s| s.to_string()) }
                    }) {
                        return Ok(id);
                    }
                }
            }
        }
        Err(Error::Provider("Yandex PDD: record not found".into()))
    }
}
