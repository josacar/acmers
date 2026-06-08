use std::collections::HashMap;

use ring::rand::SystemRandom;
use ring::signature::{RsaKeyPair, RSA_PKCS1_SHA256};
use serde_json::Value;

use crate::error::Error;
use crate::http;

use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://dns.googleapis.com/dns/v1/projects";
const DNS_SCOPE: &str = "https://www.googleapis.com/auth/ndev.clouddns.readwrite";
const DEFAULT_TOKEN_URI: &str = "https://oauth2.googleapis.com/token";

pub struct Gcloud {
    project: String,
    access_token: String,
}

impl DnsProvider for Gcloud {
    fn slug() -> &'static str {
        "gcloud"
    }

    fn env_vars() -> &'static [&'static str] {
        &["GCLOUD_PROJECT", "GCLOUD_SERVICE_ACCOUNT_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let sa_key_json = env.get("GCLOUD_SERVICE_ACCOUNT_KEY")
            .cloned()
            .or_else(|| std::env::var("GCLOUD_SERVICE_ACCOUNT_KEY").ok())
            .ok_or_else(|| Error::Config("GCLOUD_SERVICE_ACCOUNT_KEY required".into()))?;

        if sa_key_json.is_empty() {
            return Err(Error::Config("GCLOUD_SERVICE_ACCOUNT_KEY required".into()));
        }

        let sa_key: Value = serde_json::from_str(&sa_key_json)
            .map_err(|e| Error::Config(format!("GCLOUD_SERVICE_ACCOUNT_KEY invalid JSON: {e}")))?;

        let client_email = sa_key.get("client_email").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Config("service account key missing client_email".into()))?;
        let private_key_pem = sa_key.get("private_key").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Config("service account key missing private_key".into()))?;
        let token_uri = sa_key.get("token_uri").and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_TOKEN_URI);
        let key_project_id = sa_key.get("project_id").and_then(|v| v.as_str());

        let project = env.get("GCLOUD_PROJECT")
            .cloned()
            .or_else(|| std::env::var("GCLOUD_PROJECT").ok())
            .or_else(|| key_project_id.map(|s| s.to_string()))
            .ok_or_else(|| Error::Config("GCLOUD_PROJECT required (or set project_id in service account key)".into()))?;

        let access_token = get_access_token(client_email, private_key_pem, token_uri)?;

        Ok(Box::new(Gcloud { project, access_token }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_name = self.resolve_zone(domain)?;
        let auth = format!("Bearer {}", self.access_token);

        let body = serde_json::json!({
            "additions": [{
                "name": format!("{name}."),
                "type": "TXT",
                "ttl": 60,
                "rrdatas": [value],
            }]
        });
        let url = format!("{BASE_URL}/{project}/managedZones/{zone_name}/changes", project = self.project);
        let resp = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("GCloud add TXT: {e}")))?;

        if resp.status >= 300 {
            return Err(Error::Provider(format!("GCloud add TXT: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_name = match self.resolve_zone(domain) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let auth = format!("Bearer {}", self.access_token);

        let body = serde_json::json!({
            "deletions": [{
                "name": format!("{name}."),
                "type": "TXT",
                "ttl": 60,
                "rrdatas": [value],
            }]
        });
        let url = format!("{BASE_URL}/{project}/managedZones/{zone_name}/changes", project = self.project);
        let _ = http::post(&url, &serde_json::to_vec(&body).unwrap(), "application/json", &[("Authorization", &auth)]);
        Ok(())
    }
}

impl Gcloud {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let auth = format!("Bearer {}", self.access_token);
        let url = format!("{BASE_URL}/{}/managedZones", self.project);
        let resp = http::get(&url, &[("Authorization", &auth)])
            .map_err(|e| Error::Provider(format!("GCloud list zones: {e}")))?;

        if resp.status >= 300 {
            return Err(Error::Provider(format!("GCloud list zones: {} {}", resp.status, resp.body)));
        }

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("GCloud zones: {e}")))?;

        let mut search = domain.to_string();
        if !search.ends_with('.') {
            search.push('.');
        }

        let mut best_len = 0;
        let mut best_name = String::new();

        if let Some(zones) = v.get("managedZones").and_then(|z| z.as_array()) {
            for zone in zones {
                if let Some(dns_name) = zone.get("dnsName").and_then(|n| n.as_str()) {
                    if search.ends_with(dns_name) && dns_name.len() > best_len {
                        best_len = dns_name.len();
                        if let Some(name) = zone.get("name").and_then(|n| n.as_str()) {
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
}

fn get_access_token(client_email: &str, private_key_pem: &str, token_uri: &str) -> Result<String, Error> {
    let jwt = create_jwt(client_email, private_key_pem, token_uri)?;

    let body = format!(
        "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer&assertion={jwt}"
    );
    let resp = http::post(token_uri, body.as_bytes(), "application/x-www-form-urlencoded", &[])
        .map_err(|e| Error::Provider(format!("GCloud token exchange: {e}")))?;

    if resp.status >= 300 {
        return Err(Error::Provider(format!("GCloud token exchange: {} {}", resp.status, resp.body)));
    }

    let v: Value = serde_json::from_str(&resp.body)
        .map_err(|e| Error::Json(format!("GCloud token response: {e}")))?;
    v.get("access_token").and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| Error::Provider("no access_token in token response".into()))
}

fn create_jwt(client_email: &str, private_key_pem: &str, token_uri: &str) -> Result<String, Error> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error::Crypto("system time".into()))?
        .as_secs();

    let header = serde_json::json!({"alg": "RS256", "typ": "JWT"});
    let claims = serde_json::json!({
        "iss": client_email,
        "scope": DNS_SCOPE,
        "aud": token_uri,
        "iat": now,
        "exp": now + 3600,
    });

    let header_b64 = crate::base64::encode_no_pad(serde_json::to_string(&header).unwrap().as_bytes());
    let claims_b64 = crate::base64::encode_no_pad(serde_json::to_string(&claims).unwrap().as_bytes());
    let signing_input = format!("{header_b64}.{claims_b64}");

    let der = pem_to_der(private_key_pem)?;
    let key_pair = RsaKeyPair::from_pkcs8(&der)
        .map_err(|e| Error::Crypto(format!("parse service account key: {e}")))?;

    let rng = SystemRandom::new();
    let mut sig = vec![0u8; key_pair.public().modulus_len()];
    key_pair.sign(&RSA_PKCS1_SHA256, &rng, signing_input.as_bytes(), &mut sig)
        .map_err(|e| Error::Crypto(format!("JWT sign: {e}")))?;

    let sig_b64 = crate::base64::encode_no_pad(&sig);
    Ok(format!("{signing_input}.{sig_b64}"))
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
