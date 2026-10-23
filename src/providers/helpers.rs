use serde_json::Value;

use crate::error::Error;
use crate::http;

pub type Result<T> = std::result::Result<T, Error>;

pub fn find_zone_by_domain(
    base_url: &str,
    domain: &str,
    auth_headers: &[(&str, &str)],
) -> Result<String> {
    let url = base_url.to_string();
    let resp =
        http::get(&url, auth_headers).map_err(|e| Error::Provider(format!("list zones: {e}")))?;
    let v: Value =
        serde_json::from_str(&resp.body).map_err(|e| Error::Json(format!("parse zones: {e}")))?;

    if let Some(zones) = v.as_array() {
        for z in zones {
            if let Some(name) = z.get("name").and_then(|n| n.as_str())
                && (domain == name || domain.ends_with(&format!(".{name}")))
                && let Some(id) = get_zone_id(z)
            {
                return Ok(id);
            }
        }
    } else if let Some(zones) = v.get("data").and_then(|d| d.as_array()) {
        for z in zones {
            if let Some(name) = z
                .get("name")
                .or_else(|| z.get("domain"))
                .and_then(|n| n.as_str())
                && (domain == name || domain.ends_with(&format!(".{name}")))
                && let Some(id) = get_zone_id(z)
            {
                return Ok(id);
            }
        }
    } else if let Some(zones) = v.get("domains").and_then(|d| d.as_array()) {
        for z in zones {
            if let Some(name) = z
                .get("name")
                .or_else(|| z.get("domain"))
                .and_then(|n| n.as_str())
                && (domain == name || domain.ends_with(&format!(".{name}")))
                && let Some(id) = get_zone_id(z)
            {
                return Ok(id);
            }
        }
    }

    Err(Error::Provider(format!("zone not found for {domain}")))
}

pub fn find_record(
    list_url: &str,
    zone_id: &str,
    record_type: &str,
    record_name: &str,
    auth_headers: &[(&str, &str)],
) -> Result<Option<(String, Value)>> {
    let url = list_url.replace("{zone_id}", zone_id);
    let resp =
        http::get(&url, auth_headers).map_err(|e| Error::Provider(format!("list records: {e}")))?;
    let v: Value =
        serde_json::from_str(&resp.body).map_err(|e| Error::Json(format!("parse records: {e}")))?;

    let records = find_record_array(&v);
    if let Some(records) = records {
        for record in records {
            if record.get("type").and_then(|t| t.as_str()) == Some(record_type)
                && record.get("name").and_then(|n| n.as_str()) == Some(record_name)
                && let Some(id) = get_record_id(record)
            {
                return Ok(Some((id, record.clone())));
            }
        }
    }
    Ok(None)
}

pub fn create_record(
    create_url: &str,
    zone_id: &str,
    record_type: &str,
    record_name: &str,
    record_value: &str,
    ttl: u32,
    auth_headers: &[(&str, &str)],
) -> Result<String> {
    let url = create_url.replace("{zone_id}", zone_id);
    let body = serde_json::json!({
        "type": record_type,
        "name": record_name,
        "content": record_value,
        "ttl": ttl,
    });
    let resp = http::post(
        &url,
        &serde_json::to_vec(&body).unwrap(),
        "application/json",
        auth_headers,
    )
    .map_err(|e| Error::Provider(format!("create record: {e}")))?;
    let v: Value = serde_json::from_str(&resp.body)
        .map_err(|e| Error::Json(format!("parse create response: {e}")))?;

    if let Some(err) = v.get("error").and_then(|e| {
        e.get("message")
            .and_then(|m| m.as_str())
            .or_else(|| e.as_str())
    }) {
        return Err(Error::Provider(format!("create record: {err}")));
    }

    let id = get_created_id(&v)
        .ok_or_else(|| Error::Provider(format!("no record id in response: {}", resp.body)))?;
    Ok(id)
}

pub fn delete_record(
    delete_url: &str,
    zone_id: &str,
    record_id: &str,
    auth_headers: &[(&str, &str)],
) -> Result<()> {
    let url = delete_url
        .replace("{zone_id}", zone_id)
        .replace("{record_id}", record_id);
    let resp = http::delete(&url, auth_headers)
        .map_err(|e| Error::Provider(format!("delete record: {e}")))?;
    if resp.status >= 400 && resp.status != 404 {
        return Err(Error::Provider(format!(
            "delete record failed: HTTP {} {}",
            resp.status, resp.body
        )));
    }
    Ok(())
}

fn get_zone_id(v: &Value) -> Option<String> {
    v.get("id")
        .and_then(|i| i.as_str())
        .or_else(|| v.get("zone_id").and_then(|i| i.as_str()))
        .or_else(|| v.get("name").and_then(|i| i.as_str()))
        .map(|s| s.to_string())
}

fn get_record_id(v: &Value) -> Option<String> {
    v.get("id")
        .and_then(|i| if i.is_string() { i.as_str() } else { None })
        .or_else(|| v.get("record_id").and_then(|i| i.as_str()))
        .map(|s| s.to_string())
}

fn get_created_id(v: &Value) -> Option<String> {
    if let Some(id) = v.get("id").and_then(|i| i.as_str()) {
        return Some(id.to_string());
    }
    if let Some(data) = v.get("data")
        && let Some(id) = data.get("id").and_then(|i| i.as_str())
    {
        return Some(id.to_string());
    }
    if let Some(result) = v.get("result")
        && let Some(id) = result.get("id").and_then(|i| i.as_str())
    {
        return Some(id.to_string());
    }
    None
}

fn find_record_array(v: &Value) -> Option<&Vec<Value>> {
    if let Some(arr) = v.as_array() {
        return Some(arr);
    }
    v.get("data")
        .and_then(|d| d.as_array())
        .or_else(|| v.get("result").and_then(|r| r.as_array()))
        .or_else(|| v.get("records").and_then(|r| r.as_array()))
        .or_else(|| v.get("dns_records").and_then(|r| r.as_array()))
}

pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn url_encode_form(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn txt_value(value: &str) -> String {
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        value.to_string()
    } else {
        format!("\"{value}\"")
    }
}
