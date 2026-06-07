use std::collections::HashMap;

use ring::digest::{digest, SHA256};
use ring::hmac;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const ENDPOINT: &str = "https://route53.amazonaws.com/2013-04-01";
const REGION: &str = "us-east-1";
const SERVICE: &str = "route53";

pub struct Route53 {
    access_key: String,
    secret_key: String,
}

impl DnsProvider for Route53 {
    fn slug() -> &'static str {
        "aws"
    }

    fn env_vars() -> &'static [&'static str] {
        &["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let access_key = env.get("AWS_ACCESS_KEY_ID")
            .ok_or_else(|| Error::Config("AWS_ACCESS_KEY_ID required".into()))?
            .clone();
        let secret_key = env.get("AWS_SECRET_ACCESS_KEY")
            .ok_or_else(|| Error::Config("AWS_SECRET_ACCESS_KEY required".into()))?
            .clone();
        Ok(Box::new(Route53 { access_key, secret_key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.resolve_zone(domain)?;
        let body = format_xml(name, value, "UPSERT");
        let resp = self.signed_request("POST", &format!("/hostedzone/{zone_id}/rrset/"), body.as_bytes(), "application/xml")?;
        if resp.status >= 300 {
            return Err(Error::Provider(format!("Route53 add TXT: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = match self.resolve_zone(domain) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        let body = format_xml(name, value, "DELETE");
        let _ = self.signed_request("POST", &format!("/hostedzone/{zone_id}/rrset/"), body.as_bytes(), "application/xml");
        Ok(())
    }
}

fn format_xml(name: &str, value: &str, action: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ChangeResourceRecordSetsRequest xmlns="https://route53.amazonaws.com/doc/2013-04-01/">
  <ChangeBatch>
    <Changes>
      <Change>
        <Action>{action}</Action>
        <ResourceRecordSet>
          <Name>{name}</Name>
          <Type>TXT</Type>
          <TTL>60</TTL>
          <ResourceRecords>
            <ResourceRecord>
              <Value>"{value}"</Value>
            </ResourceRecord>
          </ResourceRecords>
        </ResourceRecordSet>
      </Change>
    </Changes>
  </ChangeBatch>
</ChangeResourceRecordSetsRequest>"#
    )
}

impl Route53 {
    fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
        let resp = self.signed_request("GET", "/hostedzone", b"", "application/xml")?;
        let body = &resp.body;
        let mut search = domain.to_string();
        if !search.ends_with('.') {
            search.push('.');
        }

        let mut best_len = 0;
        let mut best_id = String::new();
        let xml = body.as_bytes();
        let mut pos = 0;
        while pos < xml.len() {
            if let Some(start) = find_tag_start(xml, pos, b"<Id>") {
                let id_end = find_tag_end(xml, start, b"</Id>").unwrap_or(xml.len());
                let id_raw = &xml[start + 4..id_end];
                let id_str = std::str::from_utf8(id_raw).unwrap_or("");
                let id = id_str.strip_prefix("/hostedzone/").unwrap_or(id_str);

                let name_start = find_tag_start(xml, id_end, b"<Name>").unwrap_or(xml.len());
                let name_end = find_tag_end(xml, name_start, b"</Name>").unwrap_or(xml.len());
                let name_raw = &xml[name_start + 6..name_end];
                let name_str = std::str::from_utf8(name_raw).unwrap_or("");

                if search.ends_with(name_str) && name_str.len() > best_len {
                    best_len = name_str.len();
                    best_id = id.to_string();
                }
                pos = name_end;
            } else {
                break;
            }
        }

        if best_id.is_empty() {
            return Err(Error::Provider(format!("zone not found for {domain}")));
        }
        Ok(best_id)
    }

    fn signed_request(&self, method: &str, uri: &str, payload: &[u8], content_type: &str) -> Result<http::Response, Error> {
        let now = time::OffsetDateTime::now_utc();
        let (y, m, d) = (now.year(), now.month() as u8, now.day());
        let (h, mi, s) = (now.hour(), now.minute(), now.second());
        let timestamp = format!("{y:04}{m:02}{d:02}T{h:02}{mi:02}{s:02}Z");
        let date_stamp = format!("{y:04}{m:02}{d:02}");

        let payload_hash = base64::hex(digest(&SHA256, payload).as_ref());

        let canonical_headers = format!(
            "content-type:{content_type}\nhost:route53.amazonaws.com\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{timestamp}\n"
        );
        let signed_headers = "content-type;host;x-amz-content-sha256;x-amz-date";

        let canonical_request = format!(
            "{method}\n{uri}\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
        );
        let canonical_hash = base64::hex(digest(&SHA256, canonical_request.as_bytes()).as_ref());

        let credential_scope = format!("{date_stamp}/{REGION}/{SERVICE}/aws4_request");
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{timestamp}\n{credential_scope}\n{canonical_hash}"
        );

        let signing_key = aws_signing_key(&self.secret_key, &date_stamp, REGION, SERVICE);
        let signature_tag = hmac::sign(&signing_key, string_to_sign.as_bytes());
        let signature = base64::hex(signature_tag.as_ref());

        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}",
            self.access_key,
        );

        match method {
            "GET" => http::get(
                &format!("{ENDPOINT}{uri}"),
                &[
                    ("Content-Type", content_type),
                    ("Host", "route53.amazonaws.com"),
                    ("X-Amz-Content-Sha256", &payload_hash),
                    ("X-Amz-Date", &timestamp),
                    ("Authorization", &authorization),
                ],
            ).map_err(|e| Error::Provider(format!("Route53 request: {e}"))),
            "POST" => http::post(
                &format!("{ENDPOINT}{uri}"),
                payload,
                content_type,
                &[
                    ("Host", "route53.amazonaws.com"),
                    ("X-Amz-Content-Sha256", &payload_hash),
                    ("X-Amz-Date", &timestamp),
                    ("Authorization", &authorization),
                ],
            ).map_err(|e| Error::Provider(format!("Route53 request: {e}"))),
            _ => Err(Error::Provider(format!("unsupported method: {method}"))),
        }
    }
}

fn aws_signing_key(secret: &str, date_stamp: &str, region: &str, service: &str) -> hmac::Key {
    let k_secret = format!("AWS4{secret}");
    let k_date = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, k_secret.as_bytes()), date_stamp.as_bytes());
    let k_region = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, k_date.as_ref()), region.as_bytes());
    let k_service = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, k_region.as_ref()), service.as_bytes());
    hmac::Key::new(hmac::HMAC_SHA256, hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, k_service.as_ref()), b"aws4_request").as_ref())
}

fn find_tag_start(xml: &[u8], from: usize, tag: &[u8]) -> Option<usize> {
    let len = tag.len();
    if from + len > xml.len() {
        return None;
    }
    for i in from..xml.len() - len {
        if &xml[i..i + len] == tag {
            return Some(i);
        }
    }
    None
}

fn find_tag_end(xml: &[u8], from: usize, tag: &[u8]) -> Option<usize> {
    find_tag_start(xml, from, tag).map(|i| i + tag.len())
}
