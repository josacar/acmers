use rcgen::{KeyPair, PKCS_ECDSA_P256_SHA256};
use ring::rand::SystemRandom;
use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair as RingKeyPair};
use serde_json::{Value, json};

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
    Ok(AccountKey {
        key_pair,
        pkcs8_bytes,
        jwk,
        jwk_thumbprint: thumbprint,
    })
}

pub fn load_key_from_der(der: &[u8]) -> Result<AccountKey, Error> {
    let rng = SystemRandom::new();
    let key_pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, der, &rng)
        .map_err(|e| Error::Crypto(format!("load key: {e}")))?;

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

pub fn decode_eab_key(encoded: &str) -> Result<Vec<u8>, Error> {
    let trimmed = encoded.trim();
    if trimmed.is_empty() {
        return Err(Error::Config("EAB HMAC key is empty".into()));
    }
    let decoded = crate::base64::decode(trimmed)
        .or_else(|_| crate::base64::decode_std(trimmed))
        .map_err(|_| {
            Error::Config("EAB HMAC key is not valid base64url or standard base64".into())
        })?;
    if decoded.is_empty() {
        return Err(Error::Config("EAB HMAC key is empty".into()));
    }
    Ok(decoded)
}

pub fn eab_jws(jwk: &Value, kid: &str, hmac_key: &str, url: &str) -> Result<Value, Error> {
    let key_bytes = decode_eab_key(hmac_key)?;

    let protected = json!({"alg": "HS256", "kid": kid, "url": url});
    let protected_str = serde_json::to_string(&protected)
        .map_err(|e| Error::Crypto(format!("serialize EAB protected header: {e}")))?;
    let payload_str = serde_json::to_string(jwk)
        .map_err(|e| Error::Crypto(format!("serialize EAB payload: {e}")))?;

    let protected_b64 = crate::base64::encode_no_pad(protected_str.as_bytes());
    let payload_b64 = crate::base64::encode_no_pad(payload_str.as_bytes());
    let signing_input = format!("{protected_b64}.{payload_b64}");

    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, &key_bytes);
    let signature = ring::hmac::sign(&key, signing_input.as_bytes());

    Ok(json!({
        "protected": protected_b64,
        "payload": payload_b64,
        "signature": crate::base64::encode_no_pad(signature.as_ref()),
    }))
}

pub fn pkcs8_to_pem(der: &[u8]) -> String {
    let encoded = crate::base64::encode_std(der);
    let mut out = String::with_capacity(encoded.len() + 64);
    out.push_str("-----BEGIN PRIVATE KEY-----\n");
    for chunk in encoded.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).unwrap_or(""));
        out.push('\n');
    }
    out.push_str("-----END PRIVATE KEY-----\n");
    out
}

pub fn create_csr(domains: &[String], key_pkcs8: &[u8]) -> Result<Vec<u8>, Error> {
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

    let csr = params
        .serialize_request(&key)
        .map_err(|e| Error::Crypto(format!("CSR creation: {e}")))?;
    Ok(csr.der().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &[u8] = b"super-secret-eab-key";
    const KID: &str = "kid-1";
    const URL: &str = "https://ca.example/new-account";

    fn jwk() -> Value {
        json!({"crv": "P-256", "kty": "EC", "x": "abc", "y": "def"})
    }

    fn key_b64url() -> String {
        crate::base64::encode_no_pad(KEY)
    }

    #[test]
    fn eab_jws_has_expected_header_and_payload() {
        let eab = eab_jws(&jwk(), KID, &key_b64url(), URL).unwrap();

        let protected: Value = serde_json::from_slice(
            &crate::base64::decode(eab["protected"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(protected["alg"], "HS256");
        assert_eq!(protected["kid"], KID);
        assert_eq!(protected["url"], URL);

        let payload: Value = serde_json::from_slice(
            &crate::base64::decode(eab["payload"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(payload, jwk());
        assert!(eab["signature"].as_str().is_some());
    }

    #[test]
    fn eab_jws_signature_verifies_with_known_key() {
        let eab = eab_jws(&jwk(), KID, &key_b64url(), URL).unwrap();
        let signing_input = format!(
            "{}.{}",
            eab["protected"].as_str().unwrap(),
            eab["payload"].as_str().unwrap()
        );
        let signature = crate::base64::decode(eab["signature"].as_str().unwrap()).unwrap();
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, KEY);
        ring::hmac::verify(&key, signing_input.as_bytes(), &signature).unwrap();
    }

    #[test]
    fn eab_jws_matches_known_signature_vector() {
        let eab = eab_jws(&jwk(), KID, &key_b64url(), URL).unwrap();
        assert_eq!(
            eab["signature"].as_str().unwrap(),
            "G5NMV6cRCSbVTdoRnio9MN2P9zNsYhQVdCDXy3r93Uw"
        );
    }

    #[test]
    fn eab_key_accepts_base64url_and_standard_base64() {
        let url_safe = crate::base64::encode_no_pad(KEY);
        assert_eq!(decode_eab_key(&url_safe).unwrap(), KEY);

        let standard = crate::base64::encode_std(KEY);
        assert_eq!(decode_eab_key(&standard).unwrap(), KEY);

        let padded_standard = crate::base64::encode_std(&[0xfb, 0xff, 0xbf]);
        assert!(padded_standard.contains('+') || padded_standard.contains('/'));
        assert_eq!(
            decode_eab_key(&padded_standard).unwrap(),
            vec![0xfb, 0xff, 0xbf]
        );
    }

    #[test]
    fn eab_key_errors_do_not_leak_the_key() {
        let bad = "not valid base64 !!!";
        let err = decode_eab_key(bad).unwrap_err().to_string();
        assert!(!err.contains(bad));

        let empty = decode_eab_key("   ").unwrap_err().to_string();
        assert!(empty.contains("empty"));
    }
}
