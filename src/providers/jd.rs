use std::collections::HashMap;

use ring::digest::{digest, SHA256};
use ring::hmac;
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const JD_PROD: &str = "clouddnsservice";
const JD_API: &str = "jdcloud-api.com";
const JD_HOST: &str = "clouddnsservice.jdcloud-api.com";
const JD_API_VERSION: &str = "v1";
const JD_DEFAULT_REGION: &str = "cn-north-1";

pub struct Jd {
    access_key_id: String,
    access_key_secret: String,
    region: String,
}

impl DnsProvider for Jd {
    fn slug() -> &'static str {
        "jd"
    }

    fn env_vars() -> &'static [&'static str] {
        &["JD_ACCESS_KEY_ID", "JD_ACCESS_KEY_SECRET", "JD_REGION"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let access_key_id = env.get("JD_ACCESS_KEY_ID")
            .ok_or_else(|| Error::Config("JD_ACCESS_KEY_ID required".into()))?
            .clone();
        let access_key_secret = env.get("JD_ACCESS_KEY_SECRET")
            .ok_or_else(|| Error::Config("JD_ACCESS_KEY_SECRET required".into()))?
            .clone();
        let region = env.get("JD_REGION")
            .cloned()
            .unwrap_or_else(|| JD_DEFAULT_REGION.to_string());
        Ok(Box::new(Jd { access_key_id, access_key_secret, region }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let (domain_id, sub_domain) = self.get_root(name)?;
        let body = serde_json::json!({
            "req": {
                "hostRecord": sub_domain,
                "hostValue": value,
                "ttl": 300,
                "type": "TXT",
                "viewValue": -1
            },
            "regionId": self.region,
            "domainId": domain_id
        });
        let base_uri = format!("{JD_API_VERSION}/regions/{}/domain/{}/RRAdd", self.region, domain_id);
        let resp = self.jd_request("POST", &base_uri, "", Some(&body))?;
        if resp.body.contains("\"error\"") {
            return Err(Error::Provider(format!("JD Cloud add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, _value: &str) -> ProviderResult {
        let (domain_id, _sub_domain) = match self.get_root(name) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let body = serde_json::json!({
            "ids": [],
            "action": "del",
            "regionId": self.region,
            "domainId": domain_id
        });
        let base_uri = format!("{JD_API_VERSION}/regions/{}/domain/{}/RROperate", self.region, domain_id);
        let _ = self.jd_request("POST", &base_uri, "", Some(&body));
        Ok(())
    }
}

impl Jd {
    fn get_root(&self, fulldomain: &str) -> Result<(String, String), Error> {
        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 0..parts.len() {
            let h = parts[i..].join(".");
            if h.is_empty() {
                break;
            }
            let base_uri = format!("{JD_API_VERSION}/regions/{}/domain", self.region);
            let resp = self.jd_request("GET", &base_uri, "", None)?;
            if resp.body.contains("\"error\"") {
                return Err(Error::Provider(format!("JD Cloud list domains: {}", resp.body)));
            }
            let v: Value = serde_json::from_str(&resp.body)
                .map_err(|e| Error::Json(format!("JD Cloud domains: {e}")))?;
            if let Some(domains) = v.get("result").and_then(|r| r.get("domains")).and_then(|d| d.as_array()) {
                for d in domains {
                    let domain_name = d.get("domainName").and_then(|n| n.as_str()).unwrap_or("");
                    if domain_name == h {
                        let id = d.get("id").and_then(|i| i.as_i64()).unwrap_or(0);
                        if id != 0 {
                            let sub_domain = parts[..i].join(".");
                            return Ok((id.to_string(), sub_domain));
                        }
                    }
                }
            }
        }
        Err(Error::Provider(format!("JD Cloud: domain not found for {fulldomain}")))
    }

    fn jd_request(&self, method: &str, canonical_uri: &str, query_string: &str, data: Option<&Value>) -> Result<http::Response, Error> {
        let now = time::OffsetDateTime::now_utc();
        let (y, m, d) = (now.year(), now.month() as u8, now.day());
        let (h, mi, s) = (now.hour(), now.minute(), now.second());
        let request_date = format!("{y:04}{m:02}{d:02}T{h:02}{mi:02}{s:02}Z");
        let request_date_only = format!("{y:04}{m:02}{d:02}");

        let nonce = format!("{y:04}{m:02}{d:02}-{h:02}{mi:02}{s:02}-acmers");

        let payload_str = data.map(|d| serde_json::to_string(d).unwrap()).unwrap_or_default();
        let has_body = data.is_some();

        let (canonical_headers, signed_headers) = if has_body {
            (
                format!("content-type:application/json\nhost:{JD_HOST}\nx-jdcloud-date:{request_date}\nx-jdcloud-nonce:{nonce}\n"),
                "content-type;host;x-jdcloud-date;x-jdcloud-nonce".to_string(),
            )
        } else {
            (
                format!("host:{JD_HOST}\nx-jdcloud-date:{request_date}\nx-jdcloud-nonce:{nonce}\n"),
                "host;x-jdcloud-date;x-jdcloud-nonce".to_string(),
            )
        };

        let payload_hash = base64::hex(digest(&SHA256, payload_str.as_bytes()).as_ref());

        let canonical_request = format!(
            "{method}\n{canonical_uri}\n{query_string}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
        );
        let hashed_canonical_request = base64::hex(digest(&SHA256, canonical_request.as_bytes()).as_ref());

        let credential_scope = format!("{request_date_only}/{}/{JD_PROD}/jdcloud2_request", self.region);
        let string_to_sign = format!(
            "JDCLOUD2-HMAC-SHA256\n{request_date}\n{credential_scope}\n{hashed_canonical_request}"
        );

        let signing_key = jd_signing_key(&self.access_key_secret, &request_date_only, &self.region, JD_PROD);
        let signature_tag = hmac::sign(&signing_key, string_to_sign.as_bytes());
        let signature = base64::hex(signature_tag.as_ref());

        let authorization = format!(
            "JDCLOUD2-HMAC-SHA256 Credential={}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}",
            self.access_key_id,
        );

        let url = if query_string.is_empty() {
            format!("https://{JD_HOST}{canonical_uri}")
        } else {
            format!("https://{JD_HOST}{canonical_uri}?{query_string}")
        };

        let mut headers = vec![
            ("X-Jdcloud-Date".to_string(), request_date.clone()),
            ("X-Jdcloud-Nonce".to_string(), nonce.clone()),
            ("Authorization".to_string(), authorization.clone()),
        ];
        if has_body {
            headers.push(("Content-Type".to_string(), "application/json".to_string()));
        }

        let header_refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

        match method {
            "GET" => http::get(&url, &header_refs)
                .map_err(|e| Error::Provider(format!("JD Cloud request: {e}"))),
            "POST" => http::post(&url, payload_str.as_bytes(), "application/json", &header_refs)
                .map_err(|e| Error::Provider(format!("JD Cloud request: {e}"))),
            _ => Err(Error::Provider(format!("unsupported method: {method}"))),
        }
    }
}

fn jd_signing_key(secret: &str, date_stamp: &str, region: &str, service: &str) -> hmac::Key {
    let k_secret = format!("JDCLOUD2{secret}");
    let k_date = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, k_secret.as_bytes()), date_stamp.as_bytes());
    let k_region = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, k_date.as_ref()), region.as_bytes());
    let k_service = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, k_region.as_ref()), service.as_bytes());
    hmac::Key::new(hmac::HMAC_SHA256, hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, k_service.as_ref()), b"jdcloud2_request").as_ref())
}
