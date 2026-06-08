use std::collections::HashMap;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Miab {
    basic_auth: String,
    server: String,
}

impl Miab {
    fn resolve_zone(&self, fulldomain: &str) -> Result<String, Error> {
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let url = format!("https://{}/admin/dns/zones", self.server);
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("miab zones: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("miab zones: HTTP {} {}", resp.status, resp.body)));
        }
        let zones: Vec<String> = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Provider(format!("miab zones: {e}")))?;
        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 1..parts.len() {
            let test = parts[i..].join(".");
            if zones.iter().any(|z| z == &test) {
                return Ok(test);
            }
        }
        Err(Error::Provider(format!("miab: no zone found for {fulldomain}")))
    }
}

impl DnsProvider for Miab {
    fn slug() -> &'static str {
        "miab"
    }

    fn env_vars() -> &'static [&'static str] {
        &["MIAB_Username", "MIAB_Password", "MIAB_Server"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("MIAB_Username")
            .ok_or_else(|| Error::Config("MIAB_Username required".into()))?
            .clone();
        let password = env.get("MIAB_Password")
            .ok_or_else(|| Error::Config("MIAB_Password required".into()))?
            .clone();
        let server = env.get("MIAB_Server")
            .ok_or_else(|| Error::Config("MIAB_Server required".into()))?
            .clone();
        let creds = format!("{username}:{password}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Miab { basic_auth, server }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        self.resolve_zone(name)?;
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let url = format!("https://{}/admin/dns/custom/{}/txt", self.server, name);
        let resp = http::post(&url, value.as_bytes(), "text/plain", headers)
            .map_err(|e| Error::Provider(format!("miab add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("miab add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        self.resolve_zone(name)?;
        let headers: &[(&str, &str)] = &[("Authorization", &self.basic_auth)];
        let url = format!("https://{}/admin/dns/custom/{}/txt", self.server, name);
        let resp = http::delete_with_body(&url, value.as_bytes(), "text/plain", headers)
            .map_err(|e| Error::Provider(format!("miab remove TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("miab remove TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }
}
