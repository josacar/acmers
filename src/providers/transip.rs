use std::collections::HashMap;

use ring::digest::{digest, SHA1_FOR_LEGACY_USE_ONLY};
use ring::rand::SystemRandom;
use ring::signature::{RsaKeyPair, RSA_PKCS1_SHA512};
use serde_json::Value;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const API_URL: &str = "https://api.transip.nl/v6";

pub struct Transip {
    username: String,
    key_pair: RsaKeyPair,
}

impl DnsProvider for Transip {
    fn slug() -> &'static str {
        "transip"
    }

    fn env_vars() -> &'static [&'static str] {
        &["TRANSIP_Username", "TRANSIP_Key"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env
            .get("TRANSIP_Username")
            .ok_or_else(|| Error::Config("TRANSIP_Username required".into()))?
            .clone();

        let key_source = env
            .get("TRANSIP_Key")
            .ok_or_else(|| Error::Config("TRANSIP_Key required".into()))?;

        let pem = if key_source.contains("BEGIN PRIVATE KEY") {
            key_source.clone()
        } else if std::path::Path::new(key_source).exists() {
            std::fs::read_to_string(key_source)
                .map_err(|e| Error::Config(format!("read TransIP key file: {e}")))?
        } else {
            let decoded = base64::decode_std(key_source)
                .map_err(|e| Error::Config(format!("decode TRANSIP_Key: {e}")))?;
            String::from_utf8(decoded)
                .map_err(|e| Error::Config(format!("TRANSIP_Key not valid UTF-8: {e}")))?
        };

        let der = pem_to_der(&pem)?;
        let key_pair = RsaKeyPair::from_pkcs8(&der)
            .map_err(|e| Error::Crypto(format!("parse TransIP RSA key: {e}")))?;

        Ok(Box::new(Transip { username, key_pair }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = self.get_token()?;
        let (domain, sub_domain) = self.find_root(name, &token)?;

        let body = serde_json::json!({
            "dnsEntry": {
                "name": sub_domain,
                "type": "TXT",
                "content": value,
                "expire": 60
            }
        });
        let url = format!("{API_URL}/domains/{domain}/dns");
        let auth = format!("Bearer {token}");
        let resp = http::post(
            &url,
            &serde_json::to_vec(&body).unwrap(),
            "application/json",
            &[
                ("Authorization", &auth),
                ("Accept", "application/json"),
            ],
        )
        .map_err(|e| Error::Provider(format!("TransIP add TXT: {e}")))?;

        if resp.status >= 300 {
            return Err(Error::Provider(format!(
                "TransIP add TXT: {} {}",
                resp.status, resp.body
            )));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let token = match self.get_token() {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };
        let (domain, sub_domain) = match self.find_root(name, &token) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let body = serde_json::json!({
            "dnsEntry": {
                "name": sub_domain,
                "type": "TXT",
                "content": value,
                "expire": 60
            }
        });
        let url = format!("{API_URL}/domains/{domain}/dns");
        let auth = format!("Bearer {token}");
        let _ = http::delete_with_body(
            &url,
            &serde_json::to_vec(&body).unwrap(),
            "application/json",
            &[
                ("Authorization", &auth),
                ("Accept", "application/json"),
            ],
        );
        Ok(())
    }
}

impl Transip {
    fn get_token(&self) -> Result<String, Error> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let nonce_input = format!("TRANSIP{timestamp}\n");
        let nonce_hash = digest(&SHA1_FOR_LEGACY_USE_ONLY, nonce_input.as_bytes());
        let nonce_hex = base64::hex(nonce_hash.as_ref());
        let nonce = &nonce_hex[..32];

        let data = serde_json::json!({
            "login": self.username,
            "nonce": nonce,
            "read_only": "false",
            "expiration_time": "30 minutes",
            "label": "",
            "global_key": "false"
        });
        let data_str = serde_json::to_string(&data).unwrap();

        let signature = rsa_sign_sha512(&self.key_pair, data_str.as_bytes())?;

        let url = format!("{API_URL}/auth");
        let resp = http::post(
            &url,
            data_str.as_bytes(),
            "application/json",
            &[
                ("Signature", &signature),
                ("Accept", "application/json"),
            ],
        )
        .map_err(|e| Error::Provider(format!("TransIP auth: {e}")))?;

        if resp.status >= 300 {
            return Err(Error::Provider(format!(
                "TransIP auth failed: {} {}",
                resp.status, resp.body
            )));
        }

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("parse auth response: {e}")))?;

        v.get("token")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| Error::Provider(format!("no token in auth response: {}", resp.body)))
    }

    fn find_root(&self, fqdn: &str, token: &str) -> Result<(String, String), Error> {
        let parts: Vec<&str> = fqdn.split('.').collect();
        for i in 0..parts.len().saturating_sub(1) {
            let candidate = parts[i..].join(".");
            let sub_domain = parts[..i].join(".");
            let url = format!("{API_URL}/domains/{candidate}/dns");
            let auth = format!("Bearer {token}");
            match http::get(
                &url,
                &[
                    ("Authorization", &auth),
                    ("Accept", "application/json"),
                ],
            ) {
                Ok(resp) if resp.status < 300 && resp.body.contains("dnsEntries") => {
                    return Ok((candidate, sub_domain));
                }
                _ => continue,
            }
        }
        Err(Error::Provider(format!("zone not found for {fqdn}")))
    }
}

fn pem_to_der(pem: &str) -> Result<Vec<u8>, Error> {
    let b64: String = pem
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");
    base64::decode_std(&b64)
        .map_err(|e| Error::Crypto(format!("decode PEM: {e}")))
}

fn rsa_sign_sha512(key_pair: &RsaKeyPair, data: &[u8]) -> Result<String, Error> {
    let rng = SystemRandom::new();
    let mut sig = vec![0u8; key_pair.public().modulus_len()];
    key_pair
        .sign(&RSA_PKCS1_SHA512, &rng, data, &mut sig)
        .map_err(|e| Error::Crypto(format!("RSA sign SHA512: {e}")))?;
    Ok(base64::encode_std(&sig))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pem_to_der() {
        let pem = "-----BEGIN PRIVATE KEY-----\nMIIB\n-----END PRIVATE KEY-----";
        let der = pem_to_der(pem).unwrap();
        assert_eq!(der, base64::decode_std("MIIB").unwrap());
    }

    #[test]
    fn test_nonce_generation() {
        let timestamp = 1700000000u64;
        let nonce_input = format!("TRANSIP{timestamp}\n");
        let nonce_hash = digest(&SHA1_FOR_LEGACY_USE_ONLY, nonce_input.as_bytes());
        let nonce_hex = base64::hex(nonce_hash.as_ref());
        let nonce = &nonce_hex[..32];
        assert_eq!(nonce.len(), 32);
    }
}
