use std::collections::HashMap;

use ring::digest;
use ring::hmac;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Edgedns {
    host: String,
    access_token: String,
    client_token: String,
    client_secret: String,
}

impl DnsProvider for Edgedns {
    fn slug() -> &'static str {
        "edgedns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["AKAMAI_ACCESS_TOKEN", "AKAMAI_CLIENT_TOKEN", "AKAMAI_CLIENT_SECRET", "AKAMAI_HOST", "AKAMAI_EDGERC_CONTENT"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let host = env.get("AKAMAI_HOST")
            .ok_or_else(|| Error::Config("AKAMAI_HOST required".into()))?
            .clone();
        let access_token = env.get("AKAMAI_ACCESS_TOKEN")
            .ok_or_else(|| Error::Config("AKAMAI_ACCESS_TOKEN required".into()))?
            .clone();
        let client_token = env.get("AKAMAI_CLIENT_TOKEN")
            .ok_or_else(|| Error::Config("AKAMAI_CLIENT_TOKEN required".into()))?
            .clone();
        let client_secret = env.get("AKAMAI_CLIENT_SECRET")
            .ok_or_else(|| Error::Config("AKAMAI_CLIENT_SECRET required".into()))?
            .clone();
        Ok(Box::new(Edgedns { host, access_token, client_token, client_secret }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = self.resolve_zone(domain)?;
        let record_url = format!("https://{}/config-dns/v2/zones/{}/names/{}/types/TXT", self.host, zone, name);

        let existing = self.api_get(&record_url);
        let (method, body) = match existing {
            Ok(resp) if resp.status == 200 => {
                let v: serde_json::Value = serde_json::from_str(&resp.body)
                    .map_err(|e| Error::Json(format!("parse TXT record: {e}")))?;
                let mut rdata: Vec<String> = v.get("rdata")
                    .and_then(|r| r.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                    .unwrap_or_default();
                let quoted = format!("\"{}\"", value);
                if rdata.iter().any(|r| r == &quoted || r.trim_matches('"') == value) {
                    return Ok(());
                }
                rdata.push(quoted);
                let body = serde_json::json!({
                    "name": name,
                    "type": "TXT",
                    "ttl": 600,
                    "rdata": rdata,
                });
                ("PUT", serde_json::to_string(&body).unwrap())
            }
            _ => {
                let body = serde_json::json!({
                    "name": name,
                    "type": "TXT",
                    "ttl": 600,
                    "rdata": [format!("\"{}\"", value)],
                });
                ("POST", serde_json::to_string(&body).unwrap())
            }
        };

        let resp = self.api_request(method, &record_url, Some(body.as_bytes()))?;
        if resp.status >= 300 {
            return Err(Error::Provider(format!("EdgeDNS add TXT: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = match self.resolve_zone(domain) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let record_url = format!("https://{}/config-dns/v2/zones/{}/names/{}/types/TXT", self.host, zone, name);

        let existing = match self.api_get(&record_url) {
            Ok(resp) if resp.status == 200 => resp,
            _ => return Ok(()),
        };

        let v: serde_json::Value = serde_json::from_str(&existing.body)
            .map_err(|e| Error::Json(format!("parse TXT record: {e}")))?;
        let rdata: Vec<String> = v.get("rdata")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();

        let quoted = format!("\"{}\"", value);
        let remaining: Vec<String> = rdata.into_iter()
            .filter(|r| r != &quoted && r.trim_matches('"') != value)
            .collect();

        let resp = if remaining.is_empty() {
            self.api_delete(&record_url)?
        } else {
            let body = serde_json::json!({
                "name": name,
                "type": "TXT",
                "ttl": 600,
                "rdata": remaining,
            });
            self.api_request("PUT", &record_url, Some(serde_json::to_string(&body).unwrap().as_bytes()))?
        };

        if resp.status >= 300 && resp.status != 404 {
            return Err(Error::Provider(format!("EdgeDNS remove TXT: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }
}

impl Edgedns {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let mut cur = domain.to_string();
        loop {
            let dot_pos = cur.find('.');
            match dot_pos {
                Some(pos) => cur = cur[pos + 1..].to_string(),
                None => break,
            }
            if cur.is_empty() {
                break;
            }
            let url = format!("https://{}/config-dns/v2/zones/{}", self.host, cur);
            match self.api_get(&url) {
                Ok(resp) if resp.status == 200 => {
                    let v: serde_json::Value = serde_json::from_str(&resp.body)
                        .map_err(|e| Error::Json(format!("parse zone: {e}")))?;
                    if let Some(zone) = v.get("zone").and_then(|z| z.as_str()) {
                        return Ok(zone.to_string());
                    }
                }
                _ => continue,
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }

    fn api_get(&self, url: &str) -> Result<http::Response, Error> {
        self.api_request("GET", url, None)
    }

    fn api_delete(&self, url: &str) -> Result<http::Response, Error> {
        self.api_request("DELETE", url, None)
    }

    fn api_request(&self, method: &str, url: &str, body: Option<&[u8]>) -> Result<http::Response, Error> {
        let path = extract_path(url);
        let timestamp = edge_timestamp();
        let nonce = edge_nonce(&timestamp);

        let auth_header_base = format!(
            "EG1-HMAC-SHA256 client_token={};access_token={};timestamp={};nonce={};",
            self.client_token, self.access_token, timestamp, nonce
        );

        let content_hash = if method == "POST" {
            body.map(|b| base64_sha256(b)).unwrap_or_default()
        } else {
            String::new()
        };

        let data_to_sign = format!(
            "{}\thttps\t{}\t{}\t{}\t{}\t{}",
            method, self.host, path, "", content_hash, auth_header_base
        );

        let signing_key = make_signing_key(&self.client_secret, &timestamp);
        let signature = base64_hmac_sha256(data_to_sign.as_bytes(), &signing_key);

        let auth_header = format!("{}signature={}", auth_header_base, signature);

        let mut headers = vec![
            ("Host", self.host.as_str()),
            ("Accept", "application/json,*/*"),
            ("Authorization", auth_header.as_str()),
        ];

        let resp = match method {
            "GET" => http::get(url, &headers)
                .map_err(|e| Error::Provider(format!("EdgeDNS GET: {e}"))),
            "POST" => {
                headers.push(("Content-Type", "application/json"));
                let b = body.unwrap_or(&[]);
                http::post(url, b, "application/json", &headers)
                    .map_err(|e| Error::Provider(format!("EdgeDNS POST: {e}")))
            }
            "PUT" => {
                headers.push(("Content-Type", "application/json"));
                let b = body.unwrap_or(&[]);
                http::put(url, b, "application/json", &headers)
                    .map_err(|e| Error::Provider(format!("EdgeDNS PUT: {e}")))
            }
            "DELETE" => http::delete(url, &headers)
                .map_err(|e| Error::Provider(format!("EdgeDNS DELETE: {e}"))),
            _ => Err(Error::Provider(format!("unsupported method: {method}"))),
        };
        resp
    }
}

fn extract_path(url: &str) -> String {
    let without_scheme = url.split("://").nth(1).unwrap_or(url);
    match without_scheme.find('/') {
        Some(pos) => without_scheme[pos..].to_string(),
        None => "/".to_string(),
    }
}

fn edge_timestamp() -> String {
    let now = time::OffsetDateTime::now_utc();
    let (y, m, d) = (now.year(), now.month() as u8, now.day());
    let (h, mi, s) = (now.hour(), now.minute(), now.second());
    format!("{y:04}{m:02}{d:02}T{h:02}:{mi:02}:{s:02}+0000")
}

fn edge_nonce(timestamp: &str) -> String {
    let d = digest::digest(&digest::SHA1_FOR_LEGACY_USE_ONLY, timestamp.as_bytes());
    base64::hex(d.as_ref())[..32].to_string()
}

fn base64_sha256(data: &[u8]) -> String {
    let d = digest::digest(&digest::SHA256, data);
    base64::encode_std(d.as_ref())
}

fn make_signing_key(client_secret: &str, timestamp: &str) -> Vec<u8> {
    let key = hmac::Key::new(hmac::HMAC_SHA256, client_secret.as_bytes());
    let tag = hmac::sign(&key, timestamp.as_bytes());
    let b64 = base64::encode_std(tag.as_ref());
    b64.into_bytes()
}

fn base64_hmac_sha256(data: &[u8], key: &[u8]) -> String {
    let hmac_key = hmac::Key::new(hmac::HMAC_SHA256, key);
    let tag = hmac::sign(&hmac_key, data);
    base64::encode_std(tag.as_ref())
}
