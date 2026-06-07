use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::json as j;

pub struct Directory {
    pub new_nonce: String,
    pub new_account: String,
    pub new_order: String,
    pub new_authz: Option<String>,
    pub revoke_cert: String,
    pub key_change: String,
}

pub fn fetch(server_url: &str) -> Result<Directory, Error> {
    let resp = http::get(server_url, &[])
        .map_err(|e| Error::Config(format!("fetch directory: {e}")))?;

    let v: Value = serde_json::from_str(&resp.body)
        .map_err(|e| Error::Json(format!("parse directory: {e}")))?;

    Ok(Directory {
        new_nonce: j::get_string_required(&v, &["newNonce"])?.to_string(),
        new_account: j::get_string_required(&v, &["newAccount"])?.to_string(),
        new_order: j::get_string_required(&v, &["newOrder"])?.to_string(),
        new_authz: j::get_string(&v, &["newAuthz"]).map(|s| s.to_string()),
        revoke_cert: j::get_string_required(&v, &["revokeCert"])?.to_string(),
        key_change: j::get_string_required(&v, &["keyChange"])?.to_string(),
    })
}
