use std::collections::HashMap;

use ring::digest;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Ovh {
    ak: String,
    as_: String,
    ck: String,
    base_url: String,
}

impl DnsProvider for Ovh {
    fn slug() -> &'static str {
        "ovh"
    }

    fn env_vars() -> &'static [&'static str] {
        &["OVH_AK", "OVH_AS", "OVH_CK", "OVH_END_POINT"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let ak = env.get("OVH_AK")
            .ok_or_else(|| Error::Config("OVH_AK required".into()))?
            .clone();
        let as_ = env.get("OVH_AS")
            .ok_or_else(|| Error::Config("OVH_AS required".into()))?
            .clone();
        let ck = env.get("OVH_CK")
            .ok_or_else(|| Error::Config("OVH_CK required".into()))?
            .clone();
        let endpoint = env.get("OVH_END_POINT")
            .ok_or_else(|| Error::Config("OVH_END_POINT required".into()))?
            .clone();
        let base_url = format!("https://{endpoint}/1.0");
        Ok(Box::new(Ovh { ak, as_, ck, base_url }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = self.resolve_zone(domain)?;
        let short_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);

        let body = serde_json::json!({
            "fieldType": "TXT",
            "subDomain": short_name,
            "target": value,
            "ttl": 60,
        });
        let body_str = serde_json::to_string(&body).unwrap();
        let url_path = format!("/domain/zone/{zone}/record");
        let url = format!("{}{url_path}", self.base_url);

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        let signature = ovh_signature(&self.as_, &self.ck, "POST", &url, &body_str, &timestamp);

        let resp = http::post(
            &url,
            body_str.as_bytes(),
            "application/json",
            &[
                ("X-Ovh-Application", &self.ak),
                ("X-Ovh-Timestamp", &timestamp),
                ("X-Ovh-Consumer", &self.ck),
                ("X-Ovh-Signature", &signature),
            ],
        ).map_err(|e| Error::Provider(format!("OVH add TXT: {e}")))?;

        if resp.status >= 300 {
            return Err(Error::Provider(format!("OVH add TXT: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = match self.resolve_zone(domain) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let short_name = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);

        let record_id = match self.find_record(&zone, short_name, value) {
            Ok(Some(id)) => id,
            Ok(None) => return Ok(()),
            Err(_) => return Ok(()),
        };

        let url_path = format!("/domain/zone/{zone}/record/{record_id}");
        let url = format!("{}{url_path}", self.base_url);

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        let signature = ovh_signature(&self.as_, &self.ck, "DELETE", &url, "", &timestamp);

        let _ = http::delete(
            &url,
            &[
                ("X-Ovh-Application", &self.ak),
                ("X-Ovh-Timestamp", &timestamp),
                ("X-Ovh-Consumer", &self.ck),
                ("X-Ovh-Signature", &signature),
            ],
        );
        Ok(())
    }
}

impl Ovh {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let url_path = "/domain/zone";
        let url = format!("{}{url_path}", self.base_url);

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        let signature = ovh_signature(&self.as_, &self.ck, "GET", &url, "", &timestamp);

        let resp = http::get(
            &url,
            &[
                ("X-Ovh-Application", &self.ak),
                ("X-Ovh-Timestamp", &timestamp),
                ("X-Ovh-Consumer", &self.ck),
                ("X-Ovh-Signature", &signature),
            ],
        ).map_err(|e| Error::Provider(format!("OVH list zones: {e}")))?;

        if resp.status >= 300 {
            return Err(Error::Provider(format!("OVH list zones: {} {}", resp.status, resp.body)));
        }

        let v: serde_json::Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("OVH zones: {e}")))?;

        let mut best_len = 0;
        let mut best_name = String::new();

        if let Some(zones) = v.as_array() {
            for zone in zones {
                if let Some(name) = zone.as_str() {
                    if domain == name || domain.ends_with(&format!(".{name}")) {
                        let len = name.len();
                        if len > best_len {
                            best_len = len;
                            best_name = name.to_string();
                        }
                    }
                }
            }
        }

        if best_name.is_empty() {
            return Err(Error::Provider(format!("zone not found for {domain}")));
        }
        Ok(best_name)
    }

    fn find_record(&self, zone: &str, short_name: &str, value: &str) -> Result<Option<String>, Error> {
        let url_path = format!("/domain/zone/{zone}/record?fieldType=TXT&subDomain={short_name}");
        let url = format!("{}{url_path}", self.base_url);

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        let signature = ovh_signature(&self.as_, &self.ck, "GET", &url, "", &timestamp);

        let resp = http::get(
            &url,
            &[
                ("X-Ovh-Application", &self.ak),
                ("X-Ovh-Timestamp", &timestamp),
                ("X-Ovh-Consumer", &self.ck),
                ("X-Ovh-Signature", &signature),
            ],
        ).map_err(|e| Error::Provider(format!("OVH list records: {e}")))?;

        if resp.status >= 300 {
            return Ok(None);
        }

        let v: serde_json::Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("OVH records: {e}")))?;

        if let Some(records) = v.as_array() {
            for record in records {
                let target = record.get("target").and_then(|t| t.as_str()).unwrap_or("");
                if target == value {
                    if let Some(id) = record.get("id").and_then(|i| i.as_i64()) {
                        return Ok(Some(id.to_string()));
                    }
                }
            }
        }
        Ok(None)
    }
}

fn ovh_signature(as_: &str, ck: &str, method: &str, url: &str, body: &str, timestamp: &str) -> String {
    let to_sign = format!("{as_}+{ck}+{method}+{url}+{body}+{timestamp}");
    let d = digest::digest(&digest::SHA1_FOR_LEGACY_USE_ONLY, to_sign.as_bytes());
    format!("$1${}", base64::hex(d.as_ref()))
}
