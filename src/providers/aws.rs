use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use ring::digest::{digest, SHA256};
use ring::hmac;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const ENDPOINT: &str = "https://route53.amazonaws.com";
const REGION: &str = "us-east-1";
const SERVICE: &str = "route53";
const API_VERSION: &str = "2013-04-01";

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
        thread::sleep(Duration::from_secs(1));

        let fqdn = if name.ends_with('.') { name.to_string() } else { format!("{name}.") };
        let existing = self.get_existing_txt_values(&zone_id, &fqdn)?;
        thread::sleep(Duration::from_secs(1));

        if existing.iter().any(|v| v == value) {
            return Ok(());
        }

        let mut values = existing;
        values.push(value.to_string());
        let body = format_xml_multi(&fqdn, &values, "UPSERT");
        let resp = self.signed_request("POST", &format!("/{API_VERSION}/hostedzone/{zone_id}/rrset/"), "", body.as_bytes(), "application/xml")?;
        if resp.status >= 300 {
            return Err(Error::Provider(format!("Route53 add TXT: {} {}", resp.status, resp.body)));
        }
        thread::sleep(Duration::from_secs(1));
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = match self.resolve_zone(domain) {
            Ok(z) => z,
            Err(_) => return Ok(()),
        };
        thread::sleep(Duration::from_secs(1));

        let fqdn = if name.ends_with('.') { name.to_string() } else { format!("{name}.") };
        let existing = match self.get_existing_txt_values(&zone_id, &fqdn) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        thread::sleep(Duration::from_secs(1));

        if existing.is_empty() {
            return Ok(());
        }

        let remaining: Vec<String> = existing.into_iter().filter(|v| v != value).collect();
        if remaining.is_empty() {
            return Ok(());
        }

        let body = format_xml_multi(&fqdn, &remaining, "DELETE");
        let _ = self.signed_request("POST", &format!("/{API_VERSION}/hostedzone/{zone_id}/rrset/"), "", body.as_bytes(), "application/xml");
        thread::sleep(Duration::from_secs(1));
        Ok(())
    }
}

fn format_xml_multi(name: &str, values: &[String], action: &str) -> String {
    let records: String = values.iter().map(|v| {
        format!("<ResourceRecord><Value>\"{}\"</Value></ResourceRecord>", v)
    }).collect();
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
          <TTL>300</TTL>
          <ResourceRecords>
            {records}
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
        let mut search = domain.to_string();
        if !search.ends_with('.') {
            search.push('.');
        }

        let mut best_len = 0;
        let mut best_id = String::new();
        let mut marker: Option<String> = None;

        loop {
            let qs = match &marker {
                Some(m) => format!("marker={m}"),
                None => String::new(),
            };
            let resp = self.signed_request("GET", &format!("/{API_VERSION}/hostedzone"), &qs, b"", "application/xml")?;
            let body = &resp.body;
            let xml = body.as_bytes();

            let mut pos = 0;
            while pos < xml.len() {
                if let Some(hz_start) = find_tag_start(xml, pos, b"<HostedZone>") {
                    let hz_end = find_tag_end(xml, hz_start, b"</HostedZone>").unwrap_or(xml.len());
                    let hz_data = &xml[hz_start..hz_end];

                    let id = match extract_tag_value_str(hz_data, b"<Id>", b"</Id>") {
                        Some(id_raw) => id_raw.strip_prefix("/hostedzone/").unwrap_or(&id_raw).to_string(),
                        None => { pos = hz_end; continue; }
                    };

                    let name_str = match extract_tag_value_str(hz_data, b"<Name>", b"</Name>") {
                        Some(n) => n,
                        None => { pos = hz_end; continue; }
                    };

                    let is_private = match extract_tag_value_str(hz_data, b"<PrivateZone>", b"</PrivateZone>") {
                        Some(p) => p.eq_ignore_ascii_case("true"),
                        None => false,
                    };

                    if !is_private && search.ends_with(&name_str) && name_str.len() > best_len {
                        best_len = name_str.len();
                        best_id = id;
                    }
                    pos = hz_end;
                } else {
                    break;
                }
            }

            if contains_bytes(xml, b"<IsTruncated>true") {
                if let Some(next) = extract_tag_value_str(xml, b"<NextMarker>", b"</NextMarker>") {
                    marker = Some(next);
                    continue;
                }
            }
            break;
        }

        if best_id.is_empty() {
            return Err(Error::Provider(format!("zone not found for {domain}")));
        }
        Ok(best_id)
    }

    fn get_existing_txt_values(&self, zone_id: &str, fqdn: &str) -> Result<Vec<String>, Error> {
        let qs = format!("name={fqdn}&type=TXT");
        let resp = self.signed_request("GET", &format!("/{API_VERSION}/hostedzone/{zone_id}/rrset"), &qs, b"", "application/xml")?;
        if resp.status >= 300 {
            return Err(Error::Provider(format!("Route53 get TXT: {} {}", resp.status, resp.body)));
        }

        let xml = resp.body.as_bytes();
        let mut values = Vec::new();
        let name_tag = format!("<Name>{fqdn}</Name>");

        let mut pos = 0;
        while pos < xml.len() {
            if let Some(rrs_start) = find_tag_start(xml, pos, b"<ResourceRecordSet>") {
                let rrs_end = find_tag_end(xml, rrs_start, b"</ResourceRecordSet>").unwrap_or(xml.len());
                let rrs_data = &xml[rrs_start..rrs_end];

                if contains_bytes(rrs_data, name_tag.as_bytes()) {
                    let mut rr_pos = 0;
                    while rr_pos < rrs_data.len() {
                        if let Some(rr_start) = find_tag_start(rrs_data, rr_pos, b"<ResourceRecord>") {
                            let rr_end = find_tag_end(rrs_data, rr_start, b"</ResourceRecord>").unwrap_or(rrs_data.len());
                            let rr_data = &rrs_data[rr_start..rr_end];
                            if let Some(val) = extract_tag_value_str(rr_data, b"<Value>", b"</Value>") {
                                let cleaned = val.trim_matches('"').to_string();
                                values.push(cleaned);
                            }
                            rr_pos = rr_end;
                        } else {
                            break;
                        }
                    }
                    break;
                }
                pos = rrs_end;
            } else {
                break;
            }
        }
        Ok(values)
    }

    fn signed_request(&self, method: &str, uri: &str, query_string: &str, payload: &[u8], content_type: &str) -> Result<http::Response, Error> {
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
            "{method}\n{uri}\n{query_string}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
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

        let url = if query_string.is_empty() {
            format!("{ENDPOINT}{uri}")
        } else {
            format!("{ENDPOINT}{uri}?{query_string}")
        };

        match method {
            "GET" => http::get(
                &url,
                &[
                    ("Content-Type", content_type),
                    ("Host", "route53.amazonaws.com"),
                    ("X-Amz-Content-Sha256", &payload_hash),
                    ("X-Amz-Date", &timestamp),
                    ("Authorization", &authorization),
                ],
            ).map_err(|e| Error::Provider(format!("Route53 request: {e}"))),
            "POST" => http::post(
                &url,
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

fn extract_tag_value_str(xml: &[u8], start_tag: &[u8], end_tag: &[u8]) -> Option<String> {
    let s = find_tag_start(xml, 0, start_tag)?;
    let val_start = s + start_tag.len();
    let e = find_tag_start(xml, val_start, end_tag)?;
    std::str::from_utf8(&xml[val_start..e]).ok().map(|s| s.to_string())
}

fn contains_bytes(xml: &[u8], needle: &[u8]) -> bool {
    if needle.len() > xml.len() {
        return false;
    }
    for i in 0..=xml.len() - needle.len() {
        if &xml[i..i + needle.len()] == needle {
            return true;
        }
    }
    false
}
