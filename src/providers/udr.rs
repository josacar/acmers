use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.domainreselling.de/api/call.cgi";

pub struct Udr {
    user: String,
    pass: String,
}

impl DnsProvider for Udr {
    fn slug() -> &'static str { "udr" }
    fn env_vars() -> &'static [&'static str] { &["UDR_USER", "UDR_PASS"] }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let user = env.get("UDR_USER")
            .ok_or_else(|| Error::Config("UDR_USER required".into()))?.clone();
        let pass = env.get("UDR_PASS")
            .ok_or_else(|| Error::Config("UDR_PASS required".into()))?.clone();
        Ok(Box::new(Udr { user, pass }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = self.resolve_zone(name)
            .map_err(|e| Error::Provider(format!("udr zone: {e}")))?;
        let rr = format!("{}. 30 IN TXT {}", name, value);
        let body = format!("command=UpdateDNSZone&dnszone={}&addrr0={}", zone, rr);
        let resp = self.api_call(&body)
            .map_err(|e| Error::Provider(format!("udr add TXT: {e}")))?;
        Self::check_response(&resp.body)
            .map_err(|e| Error::Provider(format!("udr add TXT: {e}")))?;
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = match self.resolve_zone(name) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let rr = format!("{}. 30 IN TXT {}", name, value);
        let query_body = format!("command=QueryDNSZoneRRList&dnszone={}", zone);
        let resp = match self.api_call(&query_body) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if Self::check_response(&resp.body).is_err() {
            return Ok(());
        }
        if !resp.body.contains(&rr) {
            return Ok(());
        }
        let del_body = format!("command=UpdateDNSZone&dnszone={}&delrr0={}", zone, rr);
        let _ = self.api_call(&del_body);
        Ok(())
    }
}

impl Udr {
    fn api_call(&self, form_data: &str) -> Result<http::Response, String> {
        let url = format!("{}?s_login={}&s_pw={}", BASE_URL, self.user, self.pass);
        http::post(&url, form_data.as_bytes(), "application/x-www-form-urlencoded", &[])
    }

    fn check_response(body: &str) -> Result<(), String> {
        let code = body.lines()
            .find_map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with("code") {
                    trimmed.split('=').nth(1).map(|v| v.trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        if code != "200" {
            let desc = body.lines()
                .find_map(|line| {
                    let trimmed = line.trim();
                    if trimmed.starts_with("description") {
                        trimmed.split('=').nth(1).map(|v| v.trim().to_string())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| "unknown error".to_string());
            return Err(format!("API error code={}: {}", code, desc));
        }
        Ok(())
    }

    fn resolve_zone(&self, fulldomain: &str) -> Result<String, String> {
        let resp = self.api_call("command=QueryDNSZoneList")?;
        Self::check_response(&resp.body)?;
        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 0..parts.len() {
            let candidate = parts[i..].join(".");
            if candidate.is_empty() {
                continue;
            }
            if resp.body.contains(&format!("{}.", &candidate))
                || resp.body.contains(&format!("{} ", &candidate))
                || resp.body.contains(&format!("{}\t", &candidate))
            {
                return Ok(candidate);
            }
        }
        Err(format!("zone not found for {}", fulldomain))
    }
}
