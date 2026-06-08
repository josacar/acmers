use std::collections::HashMap;

use ring::digest::{digest, SHA256};
use ring::hmac;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://bcd.baidubce.com";
const HOST: &str = "bcd.baidubce.com";

pub struct Baidu {
    ak: String,
    sk: String,
}

impl DnsProvider for Baidu {
    fn slug() -> &'static str {
        "baidu"
    }

    fn env_vars() -> &'static [&'static str] {
        &["Baidu_AK", "Baidu_SK"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let ak = env.get("Baidu_AK")
            .ok_or_else(|| Error::Config("Baidu_AK required".into()))?
            .clone();
        let sk = env.get("Baidu_SK")
            .ok_or_else(|| Error::Config("Baidu_SK required".into()))?
            .clone();
        Ok(Box::new(Baidu { ak, sk }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone_name, sub_domain) = self.resolve_zone(name)?;
        let body = serde_json::json!({
            "domain": sub_domain,
            "view": "DEFAULT",
            "rdType": "TXT",
            "ttl": 300,
            "rdata": value,
            "zoneName": zone_name,
        });
        let body_str = serde_json::to_string(&body).unwrap();
        let resp = self.signed_request("POST", "/v1/domain/resolve/add", body_str.as_bytes())?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("baidu add record: HTTP {} {}", resp.status, resp.body)));
        }
        if resp.body.contains("\"code\"") && resp.body.contains("\"message\"") {
            return Err(Error::Provider(format!("baidu add record: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone_name, sub_domain) = match self.resolve_zone(name) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let ids = match self.find_record_ids(&zone_name, &sub_domain, "TXT", value) {
            Ok(ids) => ids,
            Err(_) => return Ok(()),
        };
        for id in &ids {
            let body = serde_json::json!({
                "zoneName": zone_name,
                "recordId": id.parse::<u64>().unwrap_or(0),
            });
            let body_str = serde_json::to_string(&body).unwrap();
            let _ = self.signed_request("POST", "/v1/domain/resolve/delete", body_str.as_bytes());
        }
        Ok(())
    }
}

impl Baidu {
    fn resolve_zone(&self, domain: &str) -> Result<(String, String), Error> {
        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len() {
            let candidate = parts[i..].join(".");
            let body = serde_json::json!({
                "domain": candidate,
                "pageNo": 1,
                "pageSize": 1,
            });
            let body_str = serde_json::to_string(&body).unwrap();
            let resp = match self.signed_request("POST", "/v1/domain/resolve/list", body_str.as_bytes()) {
                Ok(r) => r,
                Err(_) => continue,
            };
            if resp.status >= 400 {
                continue;
            }
            if resp.body.contains("\"totalCount\"") || resp.body.contains("\"result\"") {
                if resp.body.contains("\"code\"") && resp.body.contains("\"message\"") {
                    continue;
                }
                let sub_domain = if i == 0 {
                    "@".to_string()
                } else {
                    parts[..i].join(".")
                };
                return Ok((candidate, sub_domain));
            }
        }
        Err(Error::Provider(format!("baidu: zone not found for {domain}")))
    }

    fn find_record_ids(&self, zone_name: &str, record_domain: &str, rd_type: &str, rdata: &str) -> Result<Vec<String>, Error> {
        let mut ids = Vec::new();
        let mut page = 1;
        let page_size = 100;
        let mut max_page = 1;
        loop {
            let body = serde_json::json!({
                "domain": zone_name,
                "pageNo": page,
                "pageSize": page_size,
            });
            let body_str = serde_json::to_string(&body).unwrap();
            let resp = self.signed_request("POST", "/v1/domain/resolve/list", body_str.as_bytes())
                .map_err(|e| Error::Provider(format!("baidu list records: {e}")))?;
            if resp.status >= 400 {
                return Err(Error::Provider(format!("baidu list records: HTTP {}", resp.status)));
            }
            if resp.body.contains("\"code\"") && resp.body.contains("\"message\"") {
                return Err(Error::Provider(format!("baidu list records: {}", resp.body)));
            }
            let v: serde_json::Value = serde_json::from_str(&resp.body)
                .map_err(|e| Error::Json(format!("baidu parse records: {e}")))?;
            if page == 1 {
                if let Some(total) = v.get("totalCount").and_then(|t| t.as_u64()) {
                    max_page = ((total + page_size as u64 - 1) / page_size as u64).max(1) as usize;
                }
            }
            let records = v.get("result").and_then(|r| r.as_array())
                .or_else(|| v.get("data").and_then(|d| d.as_array()))
                .or_else(|| v.as_array());
            if let Some(records) = records {
                for record in records {
                    let rec_domain = record.get("domain").and_then(|d| d.as_str()).unwrap_or("");
                    let rec_type = record.get("rdType")
                        .or_else(|| record.get("rdtype"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("");
                    let rec_rdata = record.get("rdata").and_then(|r| r.as_str()).unwrap_or("");
                    let domain_match = rec_domain == record_domain
                        || rec_domain == format!("{record_domain}.");
                    if domain_match && rec_type == rd_type && rec_rdata == rdata {
                        if let Some(id) = record.get("recordId").and_then(|i| {
                            if let Some(n) = i.as_u64() {
                                Some(n.to_string())
                            } else {
                                i.as_str().map(|s| s.to_string())
                            }
                        }) {
                            ids.push(id);
                        }
                    }
                }
            }
            if page >= max_page {
                break;
            }
            page += 1;
        }
        Ok(ids)
    }

    fn signed_request(&self, method: &str, uri: &str, body: &[u8]) -> Result<http::Response, Error> {
        let now = time::OffsetDateTime::now_utc();
        let ts = format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            now.year(), now.month() as u8, now.day(),
            now.hour(), now.minute(), now.second()
        );
        let content_type = "application/json; charset=utf-8";
        let payload_hash = base64::hex(digest(&SHA256, body).as_ref());
        let expire = "3600";

        let auth_prefix = format!("bce-auth-v1/{}/{ts}/{expire}", self.ak);
        let signed_headers = "content-type;host;x-bce-content-sha256;x-bce-date";

        let canonical_uri = bce_encode_path(uri);
        let ct_encoded = percent_encode(content_type);
        let host_encoded = percent_encode(HOST);
        let hash_encoded = percent_encode(&payload_hash);
        let date_encoded = percent_encode(&ts);

        let canonical_headers = format!(
            "content-type:{ct_encoded}\nhost:{host_encoded}\nx-bce-content-sha256:{hash_encoded}\nx-bce-date:{date_encoded}"
        );
        let canonical_request = format!("{method}\n{canonical_uri}\n\n{canonical_headers}");

        let signing_key_tag = hmac::sign(
            &hmac::Key::new(hmac::HMAC_SHA256, self.sk.as_bytes()),
            auth_prefix.as_bytes(),
        );
        let signing_key_hex = base64::hex(signing_key_tag.as_ref());

        let signature_tag = hmac::sign(
            &hmac::Key::new(hmac::HMAC_SHA256, signing_key_hex.as_bytes()),
            canonical_request.as_bytes(),
        );
        let signature = base64::hex(signature_tag.as_ref());

        let authorization = format!("{auth_prefix}/{signed_headers}/{signature}");

        let url = format!("{BASE_URL}{uri}");
        let headers = [
            ("Authorization", authorization.as_str()),
            ("x-bce-date", ts.as_str()),
            ("x-bce-content-sha256", payload_hash.as_str()),
            ("Host", HOST),
            ("Content-Type", content_type),
        ];

        match method {
            "POST" => http::post(&url, body, content_type, &headers)
                .map_err(|e| Error::Provider(format!("baidu request: {e}"))),
            "GET" => http::get(&url, &headers)
                .map_err(|e| Error::Provider(format!("baidu request: {e}"))),
            _ => Err(Error::Provider(format!("baidu: unsupported method: {method}"))),
        }
    }
}

fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 2);
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(upper_hex(b >> 4));
                out.push(upper_hex(b & 0xf));
            }
        }
    }
    out
}

fn upper_hex(n: u8) -> char {
    if n < 10 {
        (b'0' + n) as char
    } else {
        (b'A' + n - 10) as char
    }
}

fn bce_encode_path(path: &str) -> String {
    if path.is_empty() {
        return "/".to_string();
    }
    let mut out = String::new();
    let trimmed = path.strip_prefix('/').unwrap_or(path);
    if path.starts_with('/') {
        out.push('/');
    }
    for (i, segment) in trimmed.split('/').enumerate() {
        if i > 0 {
            out.push('/');
        }
        if !segment.is_empty() {
            out.push_str(&percent_encode(segment));
        }
    }
    if out.is_empty() {
        out.push('/');
    }
    out
}
