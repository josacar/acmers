use std::collections::HashMap;

use ring::digest::{digest, SHA256};
use ring::hmac;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://dnspod.tencentcloudapi.com";
const SERVICE: &str = "dnspod";
const API_VERSION: &str = "2021-03-23";

pub struct Tencent {
    secret_id: String,
    secret_key: String,
}

impl DnsProvider for Tencent {
    fn slug() -> &'static str {
        "tencent"
    }

    fn env_vars() -> &'static [&'static str] {
        &["TENCENT_SecretId", "TENCENT_SecretKey"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let secret_id = env.get("TENCENT_SecretId")
            .ok_or_else(|| Error::Config("TENCENT_SecretId required".into()))?
            .clone();
        let secret_key = env.get("TENCENT_SecretKey")
            .ok_or_else(|| Error::Config("TENCENT_SecretKey required".into()))?
            .clone();
        Ok(Box::new(Tencent { secret_id, secret_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let short_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let _domain_id = self.resolve_domain(domain)?;

        let payload = serde_json::json!({
            "Domain": domain,
            "SubDomain": short_name,
            "RecordType": "TXT",
            "RecordLine": "默认",
            "Value": value,
            "TTL": 120,
        });
        let resp = self.signed_request("CreateRecord", &payload)?;
        let v: serde_json::Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Tencent response: {e}")))?;

        if let Some(err) = v.get("Response").and_then(|r| r.get("Error")).and_then(|e| e.get("Message")).and_then(|m| m.as_str()) {
            return Err(Error::Provider(format!("Tencent add TXT: {err}")));
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

        let payload = serde_json::json!({
            "Domain": domain,
            "RecordId": record_id.parse::<i64>().unwrap_or(0),
        });
        let _ = self.signed_request("DeleteRecord", &payload);
        Ok(())
    }
}

impl Tencent {
    fn resolve_domain(&self, domain: &str) -> Result<String, Error> {
        let payload = serde_json::json!({});
        let resp = self.signed_request("DescribeDomainList", &payload)?;
        let v: serde_json::Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Tencent domains: {e}")))?;

        if let Some(err) = v.get("Response").and_then(|r| r.get("Error")).and_then(|e| e.get("Message")).and_then(|m| m.as_str()) {
            return Err(Error::Provider(format!("Tencent list domains: {err}")));
        }

        if let Some(domains) = v.get("Response").and_then(|r| r.get("DomainList")).and_then(|d| d.as_array()) {
            for d in domains {
                if let Some(name) = d.get("Name").and_then(|n| n.as_str()) {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        if let Some(id) = d.get("DomainId").and_then(|i| i.as_i64()) {
                            return Ok(id.to_string());
                        }
                    }
                }
            }
        }

        Err(Error::Provider(format!("domain not found: {domain}")))
    }

    fn find_record(&self, domain: &str, short_name: &str, value: &str) -> Result<Option<String>, Error> {
        let payload = serde_json::json!({
            "Domain": domain,
            "Subdomain": short_name,
        });
        let resp = self.signed_request("DescribeRecordList", &payload)?;
        let v: serde_json::Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("Tencent records: {e}")))?;

        if let Some(records) = v.get("Response").and_then(|r| r.get("RecordList")).and_then(|r| r.as_array()) {
            for record in records {
                let typ = record.get("Type").and_then(|t| t.as_str()).unwrap_or("");
                let val = record.get("Value").and_then(|v| v.as_str()).unwrap_or("");
                if typ == "TXT" && val == value {
                    if let Some(id) = record.get("RecordId").and_then(|i| i.as_i64()) {
                        return Ok(Some(id.to_string()));
                    }
                }
            }
        }
        Ok(None)
    }

    fn signed_request(&self, action: &str, payload: &serde_json::Value) -> Result<http::Response, Error> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        let now = time::OffsetDateTime::now_utc();
        let date = format!("{:04}-{:02}-{:02}", now.year(), now.month() as u8, now.day());

        let payload_str = serde_json::to_string(payload).unwrap();
        let payload_hash = base64::hex(digest(&SHA256, payload_str.as_bytes()).as_ref());

        let action_lower = action.to_lowercase();
        let canonical_headers = format!("content-type:application/json; charset=utf-8\nhost:dnspod.tencentcloudapi.com\nx-tc-action:{action_lower}\n");
        let signed_headers = "content-type;host;x-tc-action";

        let canonical_request = format!(
            "POST\n/\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
        );
        let canonical_hash = base64::hex(digest(&SHA256, canonical_request.as_bytes()).as_ref());

        let credential_scope = format!("{date}/{SERVICE}/tc3_request");
        let string_to_sign = format!(
            "TC3-HMAC-SHA256\n{timestamp}\n{credential_scope}\n{canonical_hash}"
        );

        let signing_key = tc3_signing_key(&self.secret_key, &date, SERVICE);
        let signature_tag = hmac::sign(&signing_key, string_to_sign.as_bytes());
        let signature = base64::hex(signature_tag.as_ref());

        let authorization = format!(
            "TC3-HMAC-SHA256 Credential={}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}",
            self.secret_id,
        );

        http::post(
            BASE_URL,
            payload_str.as_bytes(),
            "application/json; charset=utf-8",
            &[
                ("Authorization", &authorization),
                ("Content-Type", "application/json; charset=utf-8"),
                ("Host", "dnspod.tencentcloudapi.com"),
                ("X-TC-Action", action),
                ("X-TC-Timestamp", &timestamp),
                ("X-TC-Version", API_VERSION),
            ],
        ).map_err(|e| Error::Provider(format!("Tencent request {action}: {e}")))
    }
}

fn tc3_signing_key(secret: &str, date: &str, service: &str) -> hmac::Key {
    let k_secret = format!("TC3{secret}");
    let k_date = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, k_secret.as_bytes()), date.as_bytes());
    let k_service = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, k_date.as_ref()), service.as_bytes());
    hmac::Key::new(hmac::HMAC_SHA256, hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, k_service.as_ref()), b"tc3_request").as_ref())
}
