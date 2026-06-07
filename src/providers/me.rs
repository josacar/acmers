use std::collections::HashMap;

use ring::hmac;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://api.dnsmadeeasy.com/V2.0";

fn hmac_sha1_hex(key: &[u8], data: &[u8]) -> String {
    let signing_key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, key);
    let tag = hmac::sign(&signing_key, data);
    let tag_bytes = tag.as_ref();
    let mut out = String::with_capacity(tag_bytes.len() * 2);
    for b in tag_bytes {
        out.push(HEX_LOWER[(b >> 4) as usize] as char);
        out.push(HEX_LOWER[(b & 0x0f) as usize] as char);
    }
    out
}

const HEX_LOWER: &[u8] = b"0123456789abcdef";

pub struct Dnsmadeeasy {
    api_key: String,
    api_secret: String,
}

impl DnsProvider for Dnsmadeeasy {
    fn slug() -> &'static str {
        "me"
    }

    fn env_vars() -> &'static [&'static str] {
        &["ME_Key", "ME_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("ME_Key")
            .ok_or_else(|| Error::Config("ME_Key required".into()))?
            .clone();
        let api_secret = env.get("ME_Secret")
            .ok_or_else(|| Error::Config("ME_Secret required".into()))?
            .clone();
        Ok(Box::new(Dnsmadeeasy { api_key, api_secret }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = self.resolve_domain(domain)?;
        let short_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);

        let body = serde_json::json!({
            "name": short_name,
            "type": "TXT",
            "value": value,
            "ttl": 120,
        });
        let url = format!("{BASE_URL}/dns/managed/{domain_id}/records");
        let (hmac_hex, ts) = self.make_hmac();
        let headers: &[(&str, &str)] = &[
            ("x-dnsme-apiKey", &self.api_key),
            ("x-dnsme-hmac", &hmac_hex),
            ("x-dnsme-requestDate", &ts),
        ];
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", headers)
            .map_err(|e| Error::Provider(format!("DNSMadeEasy create record: {e}")))?;

        if resp.status >= 300 {
            return Err(Error::Provider(format!("DNSMadeEasy create record: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = match self.resolve_domain(domain) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let short_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);

        let record_id = match self.find_record(&domain_id, short_name, value) {
            Ok(Some(id)) => id,
            Ok(None) => return Ok(()),
            Err(_) => return Ok(()),
        };

        let url = format!("{BASE_URL}/dns/managed/{domain_id}/records/{record_id}");
        let (hmac_hex, ts) = self.make_hmac();
        let headers: &[(&str, &str)] = &[
            ("x-dnsme-apiKey", &self.api_key),
            ("x-dnsme-hmac", &hmac_hex),
            ("x-dnsme-requestDate", &ts),
        ];
        let _ = http::delete(&url, headers);
        Ok(())
    }
}

impl Dnsmadeeasy {
    fn make_hmac(&self) -> (String, String) {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .to_string();
        let hmac_hex = hmac_sha1_hex(self.api_secret.as_bytes(), ts.as_bytes());
        (hmac_hex, ts)
    }

    fn resolve_domain(&self, domain: &str) -> Result<String, Error> {
        let (hmac_hex, ts) = self.make_hmac();
        let headers: &[(&str, &str)] = &[
            ("x-dnsme-apiKey", &self.api_key),
            ("x-dnsme-hmac", &hmac_hex),
            ("x-dnsme-requestDate", &ts),
        ];
        let url = format!("{BASE_URL}/dns/managed");
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("DNSMadeEasy list domains: {e}")))?;

        if resp.status >= 300 {
            return Err(Error::Provider(format!("DNSMadeEasy list domains: {} {}", resp.status, resp.body)));
        }

        let v: serde_json::Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("DNSMadeEasy domains: {e}")))?;

        if let Some(domains) = v.get("data").and_then(|d| d.as_array()) {
            for d in domains {
                let dname = d.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if domain == dname || domain.ends_with(&format!(".{dname}")) {
                    if let Some(id) = d.get("id").and_then(|i| i.as_i64()).map(|i| i.to_string()) {
                        return Ok(id);
                    }
                }
            }
        }
        Err(Error::Provider(format!("domain not found: {domain}")))
    }

    fn find_record(&self, domain_id: &str, short_name: &str, value: &str) -> Result<Option<String>, Error> {
        let (hmac_hex, ts) = self.make_hmac();
        let headers: &[(&str, &str)] = &[
            ("x-dnsme-apiKey", &self.api_key),
            ("x-dnsme-hmac", &hmac_hex),
            ("x-dnsme-requestDate", &ts),
        ];
        let url = format!("{BASE_URL}/dns/managed/{domain_id}/records?recordName={short_name}&type=TXT");
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("DNSMadeEasy list records: {e}")))?;

        if resp.status >= 300 {
            return Ok(None);
        }

        let v: serde_json::Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("DNSMadeEasy records: {e}")))?;

        if let Some(records) = v.get("data").and_then(|r| r.as_array()) {
            for record in records {
                let rval = record.get("value").and_then(|v| v.as_str()).unwrap_or("");
                if rval == value {
                    if let Some(id) = record.get("id").and_then(|i| i.as_i64()).map(|i| i.to_string()) {
                        return Ok(Some(id));
                    }
                }
            }
        }
        Ok(None)
    }
}
