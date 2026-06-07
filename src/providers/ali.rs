use std::collections::HashMap;

use ring::hmac;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://alidns.aliyuncs.com";
const VERSION: &str = "2015-01-09";

pub struct Aliyun {
    key: String,
    secret: String,
}

impl DnsProvider for Aliyun {
    fn slug() -> &'static str {
        "ali"
    }

    fn env_vars() -> &'static [&'static str] {
        &["Ali_Key", "Ali_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let key = env.get("Ali_Key")
            .ok_or_else(|| Error::Config("Ali_Key required".into()))?
            .clone();
        let secret = env.get("Ali_Secret")
            .ok_or_else(|| Error::Config("Ali_Secret required".into()))?
            .clone();
        Ok(Box::new(Aliyun { key, secret }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let short_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let mut params = base_params(&self.key, "AddDomainRecord");
        params.push(("DomainName".to_string(), domain.to_string()));
        params.push(("RR".to_string(), short_name.to_string()));
        params.push(("Type".to_string(), "TXT".to_string()));
        params.push(("Value".to_string(), value.to_string()));
        params.push(("TTL".to_string(), "120".to_string()));

        let signed_url = sign_request("GET", &mut params, &self.secret);
        let resp = http::get(&signed_url, &[])
            .map_err(|e| Error::Provider(format!("Aliyun add TXT: {e}")))?;

        let v: serde_json::Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Aliyun response: {e}")))?;
        if let Some(err) = v.get("Message").and_then(|m| m.as_str()) {
            if !err.is_empty() {
                return Err(Error::Provider(format!("Aliyun add TXT: {err}")));
            }
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let short_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);

        let record_id = match self.find_record(domain, short_name, value) {
            Ok(Some(id)) => id,
            Ok(None) => return Ok(()),
            Err(_) => return Ok(()),
        };

        let mut params = base_params(&self.key, "DeleteDomainRecord");
        params.push(("RecordId".to_string(), record_id));

        let signed_url = sign_request("GET", &mut params, &self.secret);
        let _ = http::get(&signed_url, &[]);
        Ok(())
    }
}

impl Aliyun {
    fn find_record(&self, domain: &str, short_name: &str, value: &str) -> Result<Option<String>, Error> {
        let mut params = base_params(&self.key, "DescribeDomainRecords");
        params.push(("DomainName".to_string(), domain.to_string()));
        params.push(("TypeKeyWord".to_string(), "TXT".to_string()));

        let signed_url = sign_request("GET", &mut params, &self.secret);
        let resp = http::get(&signed_url, &[])
            .map_err(|e| Error::Provider(format!("Aliyun list records: {e}")))?;

        let v: serde_json::Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Aliyun records: {e}")))?;

        if let Some(records) = v.get("DomainRecords").and_then(|r| r.get("Record")).and_then(|r| r.as_array()) {
            for record in records {
                let rr = record.get("RR").and_then(|r| r.as_str()).unwrap_or("");
                let typ = record.get("Type").and_then(|t| t.as_str()).unwrap_or("");
                let val = record.get("Value").and_then(|v| v.as_str()).unwrap_or("");
                if rr == short_name && typ == "TXT" && val == value {
                    if let Some(id) = record.get("RecordId").and_then(|i| i.as_str()) {
                        return Ok(Some(id.to_string()));
                    }
                }
            }
        }
        Ok(None)
    }
}

fn base_params(key: &str, action: &str) -> Vec<(String, String)> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    let nonce = ts.to_string();

    let now = time::OffsetDateTime::now_utc();
    let timestamp = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        now.year(), now.month() as u8, now.day(),
        now.hour(), now.minute(), now.second(),
    );

    vec![
        ("Action".to_string(), action.to_string()),
        ("AccessKeyId".to_string(), key.to_string()),
        ("Format".to_string(), "json".to_string()),
        ("SignatureMethod".to_string(), "HMAC-SHA1".to_string()),
        ("SignatureNonce".to_string(), nonce),
        ("SignatureVersion".to_string(), "1.0".to_string()),
        ("Timestamp".to_string(), timestamp),
        ("Version".to_string(), VERSION.to_string()),
    ]
}

fn sign_request(method: &str, params: &mut Vec<(String, String)>, secret: &str) -> String {
    params.sort_by(|a, b| a.0.cmp(&b.0));

    let mut canonical = String::new();
    for (i, (k, v)) in params.iter().enumerate() {
        if i > 0 {
            canonical.push('&');
        }
        canonical.push_str(&url_encode(k));
        canonical.push('=');
        canonical.push_str(&url_encode(v));
    }

    let string_to_sign = format!("{method}&%2F&{}", url_encode(&canonical));
    let signing_key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, format!("{secret}&").as_bytes());
    let tag = hmac::sign(&signing_key, string_to_sign.as_bytes());
    let signature = base64::encode_std(tag.as_ref());
    let encoded_sig = url_encode(&signature);

    format!("{BASE_URL}/?{canonical}&Signature={encoded_sig}")
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => {
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0x0f) as usize] as char);
            }
        }
    }
    out
}

const HEX: &[u8] = b"0123456789ABCDEF";
