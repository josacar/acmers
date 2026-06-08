use ring::rand::SystemRandom;
use ring::signature::{EcdsaKeyPair, ECDSA_P256_SHA256_FIXED_SIGNING, KeyPair as RingKeyPair};
use serde_json::{json, Value};
use rcgen::{KeyPair, PKCS_ECDSA_P256_SHA256};

use crate::error::Error;

pub struct AccountKey {
    pub key_pair: EcdsaKeyPair,
    pub pkcs8_bytes: Vec<u8>,
    pub jwk: Value,
    pub jwk_thumbprint: String,
}

pub fn generate_p256_key() -> Result<AccountKey, Error> {
    let rng = SystemRandom::new();
    let doc = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng)
        .map_err(|e| Error::Crypto(format!("generate P-256 key: {e}")))?;

    let pkcs8_bytes = doc.as_ref().to_vec();
    let key_pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &pkcs8_bytes, &rng)
        .map_err(|e| Error::Crypto(format!("load P-256 key: {e}")))?;

    let (jwk, thumbprint) = jwk_from_public_key(&key_pair);
    Ok(AccountKey { key_pair, pkcs8_bytes, jwk, jwk_thumbprint: thumbprint })
}

pub fn load_key_from_der(der: &[u8]) -> Result<AccountKey, Error> {
    let rng = SystemRandom::new();
    let key_pair = EcdsaKeyPair::from_pkcs8(
        &ECDSA_P256_SHA256_FIXED_SIGNING,
        der,
        &rng,
    ).map_err(|e| Error::Crypto(format!("load key: {e}")))?;

    let (jwk, thumbprint) = jwk_from_public_key(&key_pair);
    Ok(AccountKey {
        key_pair,
        pkcs8_bytes: der.to_vec(),
        jwk,
        jwk_thumbprint: thumbprint,
    })
}

fn jwk_from_public_key(key_pair: &EcdsaKeyPair) -> (Value, String) {
    let pub_bytes = key_pair.public_key().as_ref();
    let x = crate::base64::encode_no_pad(&pub_bytes[1..33]);
    let y = crate::base64::encode_no_pad(&pub_bytes[33..]);

    let jwk = json!({
        "kty": "EC",
        "crv": "P-256",
        "x": x,
        "y": y,
    });
    let thumbprint = jwk_thumbprint(&jwk);
    (jwk, thumbprint)
}

pub fn jwk_thumbprint(jwk: &Value) -> String {
    let canonical = serde_json::to_string(jwk).expect("JWK serialization");
    let digest = ring::digest::digest(&ring::digest::SHA256, canonical.as_bytes());
    crate::base64::encode_no_pad(digest.as_ref())
}

pub fn sign_jws(
    payload: &[u8],
    key: &EcdsaKeyPair,
    kid_or_jwk: &KidOrJwk,
    nonce: &str,
    url: &str,
) -> Result<Value, Error> {
    let rng = SystemRandom::new();
    let payload_b64 = crate::base64::encode_no_pad(payload);

    let protected = match kid_or_jwk {
        KidOrJwk::Kid(kid) => {
            json!({"alg": "ES256", "kid": kid, "nonce": nonce, "url": url})
        }
        KidOrJwk::Jwk(jwk) => {
            json!({"alg": "ES256", "jwk": jwk, "nonce": nonce, "url": url})
        }
    };

    let protected_str = serde_json::to_string(&protected)
        .map_err(|e| Error::Crypto(format!("serialize protected header: {e}")))?;
    let protected_b64 = crate::base64::encode_no_pad(protected_str.as_bytes());

    let signing_input = format!("{protected_b64}.{payload_b64}");
    let signature = key
        .sign(&rng, signing_input.as_bytes())
        .map_err(|e| Error::Crypto(format!("JWS sign: {e}")))?;
    let signature_b64 = crate::base64::encode_no_pad(signature.as_ref());

    Ok(json!({
        "protected": protected_b64,
        "payload": payload_b64,
        "signature": signature_b64,
    }))
}

pub enum KidOrJwk {
    Kid(String),
    Jwk(Value),
}

pub fn create_csr(
    domains: &[String],
    key_pkcs8: &[u8],
) -> Result<Vec<u8>, Error> {
    use rustls_pki_types::PrivatePkcs8KeyDer;
    let pkcs8_der = PrivatePkcs8KeyDer::from(key_pkcs8);
    let key = KeyPair::from_pkcs8_der_and_sign_algo(&pkcs8_der, &PKCS_ECDSA_P256_SHA256)
        .map_err(|e| Error::Crypto(format!("load key for CSR: {e}")))?;

    let mut params = rcgen::CertificateParams::new(domains.to_vec())
        .map_err(|e| Error::Crypto(format!("cert params: {e}")))?;

    params.distinguished_name.push(
        rcgen::DnType::CommonName,
        domains.first().cloned().unwrap_or_default(),
    );

    let csr = params.serialize_request(&key)
        .map_err(|e| Error::Crypto(format!("CSR creation: {e}")))?;
    Ok(csr.der().to_vec())
}
