use std::collections::HashMap;

use ring::hmac;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Cyon {
    username: String,
    password: String,
    otp_secret: String,
}

impl DnsProvider for Cyon {
    fn slug() -> &'static str {
        "cyon"
    }

    fn env_vars() -> &'static [&'static str] {
        &["CY_Username", "CY_Password", "CY_OTP_Secret"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(Cyon {
            username: env.get("CY_Username")
                .ok_or_else(|| Error::Config("CY_Username required".into()))?
                .clone(),
            password: env.get("CY_Password")
                .ok_or_else(|| Error::Config("CY_Password required".into()))?
                .clone(),
            otp_secret: env.get("CY_OTP_Secret").cloned().unwrap_or_default(),
        }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let cookies = self.login()?;
        let cookies = self.load_main_page(&cookies)?;
        let cookies = self.maybe_otp(&cookies)?;
        let domain_env = extract_domain_base(name);
        let cookies = self.change_domain_env(&domain_env, &cookies)?;
        self.add_txt_record(name, value, &cookies)?;
        let _ = http::get(
            "https://my.cyon.ch/auth/index/dologout",
            &[("Cookie", &cookies), ("X-Requested-With", "XMLHttpRequest")],
        );
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, _value: &str) -> ProviderResult {
        let cookies = match self.login() {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };
        let cookies = match self.load_main_page(&cookies) {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };
        let cookies = match self.maybe_otp(&cookies) {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };
        let domain_env = extract_domain_base(name);
        let cookies = match self.change_domain_env(&domain_env, &cookies) {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };
        let _ = self.delete_txt_records(name, &cookies);
        let _ = http::get(
            "https://my.cyon.ch/auth/index/dologout",
            &[("Cookie", &cookies), ("X-Requested-With", "XMLHttpRequest")],
        );
        Ok(())
    }
}

impl Cyon {
    fn login(&self) -> Result<String, Error> {
        let form = format!(
            "username={}&password={}&pathname=%2F",
            url_encode(&self.username),
            url_encode(&self.password),
        );
        let resp = http::post(
            "https://my.cyon.ch/auth/index/dologin-async",
            form.as_bytes(),
            "application/x-www-form-urlencoded",
            &[("X-Requested-With", "XMLHttpRequest")],
        )
        .map_err(|e| Error::Provider(format!("cyon login: {e}")))?;

        if !resp.body.contains("\"onSuccess\":\"success\"") {
            let msg = extract_json_field(&resp.body, "message")
                .unwrap_or_else(|| "unknown error".into());
            return Err(Error::Provider(format!("cyon login failed: {msg}")));
        }

        let cookies = resp
            .headers
            .get("set-cookie")
            .cloned()
            .unwrap_or_default();
        Ok(parse_cookies(&cookies))
    }

    fn load_main_page(&self, cookies: &str) -> Result<String, Error> {
        let headers: &[(&str, &str)] =
            &[("Cookie", cookies), ("X-Requested-With", "XMLHttpRequest")];
        let resp = http::get("https://my.cyon.ch/", headers)
            .map_err(|e| Error::Provider(format!("cyon main page: {e}")))?;

        update_cookies(cookies, &resp)
    }

    fn maybe_otp(&self, cookies: &str) -> Result<String, Error> {
        if self.otp_secret.is_empty() {
            return Ok(cookies.to_string());
        }

        let otp_code = generate_totp(&self.otp_secret)?;
        let form = format!("totpcode={otp_code}&pathname=%2F&rememberme=0");
        let headers: &[(&str, &str)] =
            &[("Cookie", cookies), ("X-Requested-With", "XMLHttpRequest")];
        let resp = http::post(
            "https://my.cyon.ch/auth/multi-factor/domultifactorauth-async",
            form.as_bytes(),
            "application/x-www-form-urlencoded",
            headers,
        )
        .map_err(|e| Error::Provider(format!("cyon OTP: {e}")))?;

        if !resp.body.contains("\"onSuccess\":\"success\"") {
            let msg = extract_json_field(&resp.body, "message")
                .unwrap_or_else(|| "OTP failed".into());
            return Err(Error::Provider(format!("cyon OTP failed: {msg}")));
        }

        update_cookies(cookies, &resp)
    }

    fn change_domain_env(&self, domain_env: &str, cookies: &str) -> Result<String, Error> {
        let headers: &[(&str, &str)] =
            &[("Cookie", cookies), ("X-Requested-With", "XMLHttpRequest")];
        let resp = http::get("https://my.cyon.ch/domain/", headers)
            .map_err(|e| Error::Provider(format!("cyon domain page: {e}")))?;

        if resp.body.contains("\"iserror\":true") {
            let msg = extract_json_field(&resp.body, "message")
                .unwrap_or_else(|| "unknown".into());
            return Err(Error::Provider(format!("cyon domain page error: {msg}")));
        }

        let item_key = extract_item_key(&resp.body, domain_env).ok_or_else(|| {
            Error::Provider(format!(
                "cyon: domain {domain_env} not found in domain list"
            ))
        })?;

        let env_url = format!(
            "https://my.cyon.ch/user/environment/setdomain/d/{domain_env}/gik/{item_key}"
        );
        let resp = http::get(&env_url, headers)
            .map_err(|e| Error::Provider(format!("cyon set domain env: {e}")))?;

        if resp.body.contains("multi_factor_form") {
            return Err(Error::Provider(
                "cyon: missed OTP authentication".into(),
            ));
        }

        if !resp.body.contains("\"authenticated\":true") {
            let msg = extract_json_field(&resp.body, "message")
                .unwrap_or_else(|| "unknown".into());
            return Err(Error::Provider(format!(
                "cyon domain env change failed: {msg}"
            )));
        }

        update_cookies(cookies, &resp)
    }

    fn add_txt_record(
        &self,
        fulldomain: &str,
        value: &str,
        cookies: &str,
    ) -> Result<(), Error> {
        let form = format!(
            "name={}.&ttl=900&type=TXT&dnscontent={}",
            url_encode(fulldomain),
            url_encode(value),
        );
        let headers: &[(&str, &str)] =
            &[("Cookie", cookies), ("X-Requested-With", "XMLHttpRequest")];
        let resp = http::post(
            "https://my.cyon.ch/domain/dnseditor/add-record-async",
            form.as_bytes(),
            "application/x-www-form-urlencoded",
            headers,
        )
        .map_err(|e| Error::Provider(format!("cyon add TXT: {e}")))?;

        if resp.body.contains("multi_factor_form") {
            return Err(Error::Provider(
                "cyon: missed OTP authentication".into(),
            ));
        }

        let status = extract_json_field(&resp.body, "status").unwrap_or_default();
        let valid = extract_json_field(&resp.body, "valid").unwrap_or_default();
        if status != "true" || valid != "true" {
            let msg = extract_json_field(&resp.body, "message")
                .unwrap_or_else(|| "unknown".into());
            return Err(Error::Provider(format!("cyon add TXT failed: {msg}")));
        }
        Ok(())
    }

    fn delete_txt_records(&self, fulldomain: &str, cookies: &str) -> Result<(), Error> {
        let headers: &[(&str, &str)] =
            &[("Cookie", cookies), ("X-Requested-With", "XMLHttpRequest")];
        let resp = http::get(
            "https://my.cyon.ch/domain/dnseditor/list-async",
            headers,
        )
        .map_err(|e| Error::Provider(format!("cyon list DNS: {e}")))?;

        if resp.body.contains("multi_factor_form") {
            return Err(Error::Provider(
                "cyon: missed OTP authentication".into(),
            ));
        }

        let target = format!("{fulldomain}.");
        for (hash, identifier) in parse_dns_entries(&resp.body) {
            let parts: Vec<&str> = identifier.splitn(2, '|').collect();
            if parts.len() == 2 && parts[0] == "TXT" && parts[1] == target {
                let del_form = format!(
                    "hash={}&identifier={}",
                    url_encode(&hash),
                    url_encode(&identifier),
                );
                let _ = http::post(
                    "https://my.cyon.ch/domain/dnseditor/delete-record-async",
                    del_form.as_bytes(),
                    "application/x-www-form-urlencoded",
                    headers,
                );
            }
        }
        Ok(())
    }
}

fn update_cookies(current: &str, resp: &http::Response) -> Result<String, Error> {
    let raw = resp
        .headers
        .get("set-cookie")
        .cloned()
        .unwrap_or_default();
    let new_cookies = parse_cookies(&raw);
    if new_cookies.is_empty() {
        Ok(current.to_string())
    } else if current.is_empty() {
        Ok(new_cookies)
    } else {
        Ok(format!("{current}; {new_cookies}"))
    }
}

fn extract_json_field(body: &str, field: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    match v.get(field)? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn extract_item_key(html: &str, domain: &str) -> Option<String> {
    let flat = html.replace('\n', " ").replace('\r', " ");
    let marker = format!("data-domain=\"{domain}\"");
    let pos = flat.find(&marker)?;
    let after = &flat[pos + marker.len()..];
    let boundary = after.find('<')?;
    let segment = &after[..boundary];
    let key_marker = "data-itemkey=\"";
    let key_pos = segment.find(key_marker)?;
    let key_start = key_pos + key_marker.len();
    let key_end = segment[key_start..].find('"')?;
    Some(segment[key_start..key_start + key_end].to_string())
}

fn parse_dns_entries(html: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut search = html;
    loop {
        let hash_attr = "data-hash=\"";
        let Some(hash_pos) = search.find(hash_attr) else {
            break;
        };
        let hash_start = hash_pos + hash_attr.len();
        let Some(hash_end) = search[hash_start..].find('"') else {
            break;
        };
        let hash = &search[hash_start..hash_start + hash_end];
        let after_hash = &search[hash_start + hash_end + 1..];
        let id_attr = "data-identifier=\"";
        let Some(id_pos) = after_hash.find(id_attr) else {
            break;
        };
        let id_start = id_pos + id_attr.len();
        let Some(id_end) = after_hash[id_start..].find('"') else {
            break;
        };
        let identifier = &after_hash[id_start..id_start + id_end];
        entries.push((hash.to_string(), identifier.to_string()));
        search = &after_hash[id_start + id_end + 1..];
    }
    entries
}

fn extract_domain_base(fulldomain: &str) -> String {
    let parts: Vec<&str> = fulldomain.split('.').collect();
    if parts.len() >= 2 {
        format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1])
    } else {
        fulldomain.to_string()
    }
}

fn generate_totp(secret_base32: &str) -> Result<String, Error> {
    let key = base32_decode(secret_base32)
        .ok_or_else(|| Error::Provider("cyon: invalid base32 OTP secret".into()))?;

    let time = time::OffsetDateTime::now_utc().unix_timestamp();
    let counter = (time / 30) as u64;
    let counter_bytes = counter.to_be_bytes();

    let hmac_key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, &key);
    let tag = hmac::sign(&hmac_key, &counter_bytes);
    let hash = tag.as_ref();

    let offset = (hash[hash.len() - 1] & 0x0f) as usize;
    let code = ((hash[offset] as u32 & 0x7f) << 24)
        | ((hash[offset + 1] as u32) << 16)
        | ((hash[offset + 2] as u32) << 8)
        | (hash[offset + 3] as u32);
    let otp = code % 1_000_000;

    Ok(format!("{otp:06}"))
}

fn base32_decode(input: &str) -> Option<Vec<u8>> {
    let input = input.trim().to_uppercase();
    let input = input.trim_end_matches('=');
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut bits = 0u64;
    let mut bit_count = 0;
    let mut output = Vec::new();

    for c in input.bytes() {
        let val = alphabet.iter().position(|&b| b == c)?;
        bits = (bits << 5) | val as u64;
        bit_count += 5;
        if bit_count >= 8 {
            bit_count -= 8;
            output.push((bits >> bit_count) as u8);
            bits &= (1 << bit_count) - 1;
        }
    }
    Some(output)
}

fn parse_cookies(raw: &str) -> String {
    let mut cookies = Vec::new();
    for part in raw.split(',') {
        let part = part.trim();
        if let Some(semi) = part.find(';') {
            if part[..semi].contains('=') {
                cookies.push(part[..semi].to_string());
            }
        } else if part.contains('=') {
            cookies.push(part.to_string());
        }
    }
    cookies.join("; ")
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push(hex((b >> 4) & 0xf));
                out.push(hex(b & 0xf));
            }
        }
    }
    out
}

fn hex(n: u8) -> char {
    if n < 10 {
        (b'0' + n) as char
    } else {
        (b'A' + n - 10) as char
    }
}
