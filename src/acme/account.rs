use serde_json::Value;

use crate::config::Config;
use crate::crypto;
use crate::error::Error;
use crate::http;
use crate::json as j;

pub struct Account {
    pub url: String,
    pub key_b64: String,
    pub jwk_thumbprint: String,
    pub server: String,
}

pub fn load_or_register(config: &Config, email: &str) -> Result<Account, Error> {
    let account_path = config.account_file();
    if account_path.exists() {
        let data = std::fs::read_to_string(&account_path)?;
        let v: Value = serde_json::from_str(&data)
            .map_err(|e| Error::Config(format!("parse account: {e}")))?;
        return Ok(Account {
            url: j::get_string_required(&v, &["url"])?.to_string(),
            key_b64: j::get_string_required(&v, &["key"])?.to_string(),
            jwk_thumbprint: j::get_string_required(&v, &["jwk_thumbprint"])?.to_string(),
            server: j::get_string(&v, &["server"]).unwrap_or(&config.server).to_string(),
        });
    }

    let dir = super::directory::fetch(&config.server)?;
    let account = register(email, &dir.new_nonce, &dir.new_account)?;
    let data = serde_json::json!({
        "url": account.url,
        "key": account.key_b64,
        "jwk_thumbprint": account.jwk_thumbprint,
        "server": config.server,
    });
    std::fs::write(&account_path, serde_json::to_string_pretty(&data).unwrap())?;
    Ok(account)
}

fn register(email: &str, nonce_url: &str, account_url: &str) -> Result<Account, Error> {
    let ak = crypto::generate_p256_key()?;
    let key_b64 = crate::base64::encode_no_pad(&ak.pkcs8_bytes);
    let thumbprint = ak.jwk_thumbprint.clone();

    let nonce = get_nonce(nonce_url)?;

    let payload = serde_json::json!({
        "termsOfServiceAgreed": true,
        "contact": [format!("mailto:{email}")],
    });

    let jws = crypto::sign_jws(
        &serde_json::to_vec(&payload).unwrap(),
        &ak.key_pair,
        &crypto::KidOrJwk::Jwk(ak.jwk),
        &nonce,
        account_url,
    )?;

    let resp = http::post(
        account_url,
        &serde_json::to_vec(&jws).unwrap(),
        "application/jose+json",
        &[],
    )
    .map_err(|e| Error::Acme { status: 0, detail: e, error_type: "http".into() })?;

    if resp.status != 200 && resp.status != 201 {
        return Err(Error::Acme {
            status: resp.status,
            detail: resp.body.clone(),
            error_type: "registration_failed".into(),
        });
    }

    let body_v: Value = serde_json::from_str(&resp.body)
        .map_err(|e| Error::Json(format!("parse account response: {e}")))?;

    let kid = resp.headers.get("location")
        .cloned()
        .or_else(|| j::get_string(&body_v, &["contact"]).map(|s| format!("{account_url}/{s}")))
        .unwrap_or_else(|| account_url.to_string());

    Ok(Account {
        url: kid,
        key_b64,
        jwk_thumbprint: thumbprint,
        server: String::new(),
    })
}

fn get_nonce(url: &str) -> Result<String, Error> {
    let resp = http::head(url)
        .map_err(|e| Error::Config(format!("get nonce: {e}")))?;
    resp.headers.get("replay-nonce")
        .or_else(|| resp.headers.get("Replay-Nonce"))
        .cloned()
        .ok_or_else(|| Error::Acme {
            status: resp.status,
            detail: "no Replay-Nonce in response".into(),
            error_type: "missing_nonce".into(),
        })
}

pub fn key_authorization(token: &str, jwk_thumbprint: &str) -> String {
    format!("{token}.{jwk_thumbprint}")
}

pub fn dns_txt_value(token: &str, jwk_thumbprint: &str) -> String {
    let ka = key_authorization(token, jwk_thumbprint);
    let digest = ring::digest::digest(&ring::digest::SHA256, ka.as_bytes());
    crate::base64::encode_no_pad(digest.as_ref())
}
