use std::collections::HashMap;

use ring::rand::SystemRandom;
use ring::signature::{RsaKeyPair, RSA_PSS_SHA256};
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Yc {
    sa_id: String,
    sa_key_id: String,
    key_pem: String,
    folder_id: Option<String>,
    zone_id: Option<String>,
}

impl DnsProvider for Yc {
    fn slug() -> &'static str {
        "yc"
    }

    fn env_vars() -> &'static [&'static str] {
        &["YC_SA_ID", "YC_SA_Key_ID", "YC_SA_Key_File_Path", "YC_Folder_ID", "YC_Zone_ID"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let sa_id = env.get("YC_SA_ID")
            .ok_or_else(|| Error::Config("YC_SA_ID required".into()))?
            .clone();
        let sa_key_id = env.get("YC_SA_Key_ID")
            .ok_or_else(|| Error::Config("YC_SA_Key_ID required".into()))?
            .clone();
        let key_file_path = env.get("YC_SA_Key_File_Path")
            .ok_or_else(|| Error::Config("YC_SA_Key_File_Path required".into()))?;
        let key_pem = std::fs::read_to_string(key_file_path)
            .map_err(|e| Error::Config(format!("YC_SA_Key_File_Path: {e}")))?;
        let folder_id = env.get("YC_Folder_ID").cloned();
        let zone_id = env.get("YC_Zone_ID").cloned();

        if folder_id.is_none() && zone_id.is_none() {
            return Err(Error::Config("YC_Folder_ID or YC_Zone_ID required".into()));
        }

        Ok(Box::new(Yc { sa_id, sa_key_id, key_pem, folder_id, zone_id }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = self.get_iam_token()?;
        let auth = format!("Bearer {token}");
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let (zone_id, sub_domain) = self.resolve_zone(domain, name, headers)?;

        let body = serde_json::to_vec(&serde_json::json!({
            "merges": [{
                "name": sub_domain,
                "type": "TXT",
                "ttl": 120,
                "data": [value],
            }]
        })).unwrap();
        let url = format!("https://dns.api.cloud.yandex.net/dns/v1/zones/{zone_id}:upsertRecordSets");
        let resp = http::post(&url, &body, "application/json", headers)
            .map_err(|e| Error::Provider(format!("YC add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("YC add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, _value: &str) -> ProviderResult {
        let token = match self.get_iam_token() {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };
        let auth = format!("Bearer {token}");
        let headers: &[(&str, &str)] = &[("Authorization", &auth)];
        let (zone_id, sub_domain) = match self.resolve_zone(domain, name, headers) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };

        let get_url = format!(
            "https://dns.api.cloud.yandex.net/dns/v1/zones/{zone_id}:getRecordSet?type=TXT&name={sub_domain}"
        );
        let resp = match http::get(&get_url, headers) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };
        if resp.status >= 400 {
            return Ok(());
        }

        let v: Value = match serde_json::from_str(&resp.body) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let data = v.get("data").cloned().unwrap_or(serde_json::json!([]));
        let ttl = v.get("ttl").and_then(|t| t.as_u64()).unwrap_or(120);

        let body = serde_json::to_vec(&serde_json::json!({
            "deletions": [{
                "name": sub_domain,
                "type": "TXT",
                "ttl": ttl,
                "data": data,
            }]
        })).unwrap();
        let url = format!("https://dns.api.cloud.yandex.net/dns/v1/zones/{zone_id}:updateRecordSets");
        let _ = http::post(&url, &body, "application/json", headers);
        Ok(())
    }
}

impl Yc {
    fn get_iam_token(&self) -> Result<String, Error> {
        let jwt = self.create_jwt()?;
        let body = serde_json::to_vec(&serde_json::json!({"jwt": jwt})).unwrap();
        let resp = http::post(
            "https://iam.api.cloud.yandex.net/iam/v1/tokens",
            &body,
            "application/json",
            &[],
        )
        .map_err(|e| Error::Provider(format!("YC IAM auth: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!(
                "YC IAM auth: HTTP {} {}",
                resp.status, resp.body
            )));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("YC IAM auth: {e}")))?;
        let token = v
            .get("iamToken")
            .and_then(|t| t.as_str())
            .ok_or_else(|| {
                Error::Provider(format!("YC IAM auth: no token: {}", resp.body))
            })?;
        Ok(token.to_string())
    }

    fn create_jwt(&self) -> Result<String, Error> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Error::Crypto("system time".into()))?
            .as_secs();

        let header = serde_json::json!({"typ": "JWT", "alg": "PS256", "kid": self.sa_key_id});
        let payload = serde_json::json!({
            "iss": self.sa_id,
            "aud": "https://iam.api.cloud.yandex.net/iam/v1/tokens",
            "iat": now,
            "exp": now + 1200,
        });

        let header_b64 = crate::base64::encode_no_pad(
            serde_json::to_string(&header).unwrap().as_bytes(),
        );
        let payload_b64 = crate::base64::encode_no_pad(
            serde_json::to_string(&payload).unwrap().as_bytes(),
        );
        let signing_input = format!("{header_b64}.{payload_b64}");

        let der = pem_to_der(&self.key_pem)?;
        let key_pair = RsaKeyPair::from_pkcs8(&der)
            .map_err(|e| Error::Crypto(format!("YC parse key: {e}")))?;

        let rng = SystemRandom::new();
        let mut sig = vec![0u8; key_pair.public().modulus_len()];
        key_pair
            .sign(
                &RSA_PSS_SHA256,
                &rng,
                signing_input.as_bytes(),
                &mut sig,
            )
            .map_err(|e| Error::Crypto(format!("YC JWT sign: {e}")))?;

        let sig_b64 = crate::base64::encode_no_pad(&sig);
        Ok(format!("{signing_input}.{sig_b64}"))
    }

    fn resolve_zone(
        &self,
        domain: &str,
        name: &str,
        headers: &[(&str, &str)],
    ) -> Result<(String, String), Error> {
        if let Some(ref zone_id) = self.zone_id {
            let url = format!(
                "https://dns.api.cloud.yandex.net/dns/v1/zones/{zone_id}"
            );
            let resp = http::get(&url, headers)
                .map_err(|e| Error::Provider(format!("YC get zone: {e}")))?;
            if resp.status >= 400 {
                return Err(Error::Provider(format!(
                    "YC get zone: HTTP {} {}",
                    resp.status, resp.body
                )));
            }
            let v: Value = serde_json::from_str(&resp.body)
                .map_err(|e| Error::Json(format!("YC zone: {e}")))?;
            let zone_name = v
                .get("zone")
                .and_then(|z| z.as_str())
                .ok_or_else(|| Error::Provider("YC zone: no zone field".into()))?;
            let sub_domain = compute_sub_domain(name, zone_name);
            return Ok((zone_id.clone(), sub_domain));
        }

        let folder_id = self
            .folder_id
            .as_ref()
            .ok_or_else(|| Error::Config("YC_Folder_ID required".into()))?;
        let url = format!(
            "https://dns.api.cloud.yandex.net/dns/v1/zones?folderId={folder_id}"
        );
        let resp = http::get(&url, headers)
            .map_err(|e| Error::Provider(format!("YC list zones: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!(
                "YC list zones: HTTP {} {}",
                resp.status, resp.body
            )));
        }
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("YC zones: {e}")))?;

        let fulldomain = format!("{name}.");
        if let Some(zones) = v.get("dnsZones").and_then(|z| z.as_array()) {
            for zone in zones {
                let zone_name =
                    zone.get("zone").and_then(|z| z.as_str()).unwrap_or("");
                if zone_name.is_empty() {
                    continue;
                }
                if fulldomain.ends_with(zone_name) || fulldomain == zone_name {
                    if let Some(id) = zone.get("id").and_then(|i| i.as_str()) {
                        let sub_domain = compute_sub_domain(name, zone_name);
                        return Ok((id.to_string(), sub_domain));
                    }
                }
            }
        }
        Err(Error::Provider(format!("zone not found for {domain}")))
    }
}

fn compute_sub_domain(name: &str, zone_name: &str) -> String {
    let fulldomain = format!("{name}.");
    let zone = if zone_name.ends_with('.') {
        zone_name.to_string()
    } else {
        format!("{zone_name}.")
    };
    if fulldomain.ends_with(&zone) {
        let cut = fulldomain.len() - zone.len();
        let sub = fulldomain[..cut].trim_end_matches('.');
        sub.to_string()
    } else {
        name.to_string()
    }
}

fn pem_to_der(pem: &str) -> Result<Vec<u8>, Error> {
    let b64: String = pem
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");
    crate::base64::decode_std(&b64)
        .map_err(|e| Error::Crypto(format!("decode private key PEM: {e}")))
}
