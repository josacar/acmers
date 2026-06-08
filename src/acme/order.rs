use serde_json::Value;

use crate::crypto::{AccountKey, KidOrJwk, sign_jws};
use crate::error::Error;
use crate::http;
use crate::json as j;

fn check_response(resp: &http::Response, context: &str) -> Result<(), Error> {
    if resp.status >= 400 {
        let v: Value = serde_json::from_str(&resp.body).unwrap_or(Value::Null);
        let detail = j::get_string(&v, &["detail"]).unwrap_or(&resp.body);
        let typ = j::get_string(&v, &["type"]).unwrap_or("about:blank");
        return Err(Error::Acme {
            status: resp.status,
            detail: format!("{context}: {detail}"),
            error_type: typ.to_string(),
        });
    }
    Ok(())
}

pub struct Order {
    pub url: String,
    pub status: String,
    pub expires: Option<String>,
    pub identifiers: Vec<String>,
    pub authorizations: Vec<String>,
    pub finalize: String,
    pub certificate: Option<String>,
}

pub fn create_order(
    domains: &[String],
    account_url: &str,
    new_order_url: &str,
    key: &AccountKey,
    nonce: &str,
) -> Result<Order, Error> {
    let identifiers: Vec<Value> = domains.iter()
        .map(|d| serde_json::json!({"type": "dns", "value": d}))
        .collect();

    let payload = serde_json::json!({"identifiers": identifiers});

    let jws = sign_jws(
        &serde_json::to_vec(&payload).unwrap(),
        &key.key_pair,
        &KidOrJwk::Kid(account_url.to_string()),
        nonce,
        new_order_url,
    )?;

    let resp = http::post(
        new_order_url,
        &serde_json::to_vec(&jws).unwrap(),
        "application/jose+json",
        &[],
    )
    .map_err(|e| Error::Acme { status: 0, detail: e, error_type: "http".into() })?;

    check_response(&resp, "create order")?;

    let v: Value = serde_json::from_str(&resp.body)
        .map_err(|e| Error::Json(format!("create order: {e} (body: {})", &resp.body[..resp.body.len().min(500)])))?;

    let order_url = j::get_string(&v, &["url"])
        .or_else(|| resp.headers.get("location").map(|s| s.as_str()))
        .unwrap_or(new_order_url)
        .to_string();

    let status = j::get_string_required(&v, &["status"])?.to_string();
    let auths = j::get_array_required(&v, &["authorizations"])?;
    let finalize = j::get_string_required(&v, &["finalize"])?.to_string();
    let cert = j::get_string(&v, &["certificate"]).map(|s| s.to_string());

    let identifiers_list = j::get_array(&v, &["identifiers"])
        .map(|arr| {
            arr.iter()
                .filter_map(|id| j::get_string(id, &["value"]))
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();

    Ok(Order {
        url: order_url,
        status,
        expires: j::get_string(&v, &["expires"]).map(|s| s.to_string()),
        identifiers: identifiers_list,
        authorizations: auths.iter().filter_map(|a| a.as_str().map(|s| s.to_string())).collect(),
        finalize,
        certificate: cert,
    })
}

pub fn finalize_order(
    csr_der: &[u8],
    finalize_url: &str,
    account_url: &str,
    key: &AccountKey,
    get_nonce: &mut dyn FnMut() -> Result<String, Error>,
) -> Result<String, Error> {
    let csr_b64 = crate::base64::encode_no_pad(csr_der);
    let payload = serde_json::json!({"csr": csr_b64});

    let nonce = get_nonce()?;
    let jws = sign_jws(
        &serde_json::to_vec(&payload).unwrap(),
        &key.key_pair,
        &KidOrJwk::Kid(account_url.to_string()),
        &nonce,
        finalize_url,
    )?;

    let resp = http::post(
        finalize_url,
        &serde_json::to_vec(&jws).unwrap(),
        "application/jose+json",
        &[],
    )
    .map_err(|e| Error::Acme { status: 0, detail: e, error_type: "http".into() })?;

    check_response(&resp, "finalize order")?;

    let v: Value = serde_json::from_str(&resp.body)
        .map_err(|e| Error::Json(format!("finalize order: {e}")))?;

    let status = j::get_string_required(&v, &["status"])?;
    if status == "invalid" {
        let err = j::get_string(&v, &["error", "detail"]).unwrap_or("finalize failed");
        return Err(Error::Acme {
            status: resp.status,
            detail: err.to_string(),
            error_type: "finalize_failed".into(),
        });
    }

    let order_url = j::get_string(&v, &["url"])
        .or_else(|| resp.headers.get("location").map(|s| s.as_str()))
        .unwrap_or(finalize_url)
        .to_string();

    poll_order(&order_url, account_url, key, get_nonce)
}

pub fn poll_order(
    order_url: &str,
    account_url: &str,
    key: &AccountKey,
    get_nonce: &mut dyn FnMut() -> Result<String, Error>,
) -> Result<String, Error> {
    for _ in 0..60 {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let nonce = get_nonce()?;
        let jws = sign_jws(
            b"",
            &key.key_pair,
            &KidOrJwk::Kid(account_url.to_string()),
            &nonce,
            order_url,
        )?;
        let resp = http::post(
            order_url,
            &serde_json::to_vec(&jws).unwrap(),
            "application/jose+json",
            &[],
        )
        .map_err(|e| Error::Acme { status: 0, detail: e, error_type: "http".into() })?;

        if resp.status >= 400 { continue; }

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("poll order: {e}")))?;

        let status = j::get_string_required(&v, &["status"])?;
        match status {
            "valid" => {
                let cert_url = j::get_string_required(&v, &["certificate"])?;
                return Ok(cert_url.to_string());
            }
            "invalid" => {
                let err = j::get_string(&v, &["error", "detail"]).unwrap_or("order invalid");
                return Err(Error::Acme {
                    status: resp.status,
                    detail: err.to_string(),
                    error_type: "order_failed".into(),
                });
            }
            _ => continue,
        }
    }
    Err(Error::Acme {
        status: 0,
        detail: "timed out waiting for order finalization".into(),
        error_type: "timeout".into(),
    })
}

pub fn download_cert(
    cert_url: &str,
    account_url: &str,
    key: &AccountKey,
    get_nonce: &mut dyn FnMut() -> Result<String, Error>,
) -> Result<String, Error> {
    let nonce = get_nonce()?;
    let jws = sign_jws(
        b"",
        &key.key_pair,
        &KidOrJwk::Kid(account_url.to_string()),
        &nonce,
        cert_url,
    )?;
    let resp = http::post(
        cert_url,
        &serde_json::to_vec(&jws).unwrap(),
        "application/jose+json",
        &[],
    )
    .map_err(|e| Error::Acme { status: 0, detail: e, error_type: "http".into() })?;

    check_response(&resp, "download certificate")?;
    Ok(resp.body)
}
