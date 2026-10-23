use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::acme::directory::Directory;
use crate::base64;
use crate::error::Error;
use crate::http;
use crate::json as j;

pub struct SuggestedWindow {
    pub start: OffsetDateTime,
    pub end: OffsetDateTime,
}

pub fn fetch(directory: &Directory, cert_pem: &str) -> Option<SuggestedWindow> {
    let url = directory.renewal_info.as_ref()?;
    let der = first_certificate_der(cert_pem)?;
    let (_, cert) = x509_parser::parse_x509_certificate(&der).ok()?;
    let serial = cert.tbs_certificate.serial.to_bytes_be();
    let aki = cert.extensions().iter().find_map(|ext| {
        if let x509_parser::extensions::ParsedExtension::AuthorityKeyIdentifier(aki) =
            ext.parsed_extension()
        {
            aki.key_identifier.as_ref().map(|ki| ki.0.to_vec())
        } else {
            None
        }
    })?;
    let cert_id = format!(
        "{}.{}",
        base64::encode_no_pad(&aki),
        base64::encode_no_pad(&serial)
    );
    let endpoint = format!("{}/{}", url.trim_end_matches('/'), cert_id);
    let resp = match http::get(&endpoint, &[]) {
        Ok(resp) if resp.status < 400 => resp,
        _ => return None,
    };
    let v: Value = serde_json::from_str(&resp.body).ok()?;
    let start = j::get_string(&v, &["suggestedWindow", "start"])?;
    let end = j::get_string(&v, &["suggestedWindow", "end"])?;
    Some(SuggestedWindow {
        start: OffsetDateTime::parse(start, &Rfc3339).ok()?,
        end: OffsetDateTime::parse(end, &Rfc3339).ok()?,
    })
}

pub fn is_due(window: &SuggestedWindow) -> bool {
    OffsetDateTime::now_utc() >= window.start
}

pub fn is_before_window(window: &SuggestedWindow) -> bool {
    OffsetDateTime::now_utc() < window.start
}

fn first_certificate_der(pem: &str) -> Option<Vec<u8>> {
    let normalized = pem.replace("\r\n", "\n");
    let begin = "-----BEGIN CERTIFICATE-----";
    let end = "-----END CERTIFICATE-----";
    let start = normalized.find(begin)?;
    let after = &normalized[start..];
    let end_pos = after.find(end)? + end.len();
    let body: String = after[..end_pos]
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    base64::decode_std(&body).ok()
}

pub fn print_error(e: &Error) {
    eprintln!("warning: ARI lookup failed: {e}");
}
