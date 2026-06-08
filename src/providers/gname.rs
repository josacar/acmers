use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

const API_BASE: &str = "https://api.gname.com";
const TLD_URL: &str = "https://www.gname.com/request/tlds?lx=all";

pub struct Gname {
    appid: String,
    appkey: String,
}

impl DnsProvider for Gname {
    fn slug() -> &'static str {
        "gname"
    }

    fn env_vars() -> &'static [&'static str] {
        &["GNAME_APPID", "GNAME_APPKEY"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let appid = env.get("GNAME_APPID")
            .ok_or_else(|| Error::Config("GNAME_APPID required".into()))?
            .clone();
        let appkey = env.get("GNAME_APPKEY")
            .ok_or_else(|| Error::Config("GNAME_APPKEY required".into()))?
            .clone();
        Ok(Box::new(Gname { appid, appkey }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (ext_domain, ext_hostname) = resolve_zone(name)
            .map_err(|e| Error::Provider(format!("gname zone: {e}")))?;
        let hostname = if ext_hostname.is_empty() { "@" } else { &ext_hostname };
        let encoded_value = url_encode(value);
        let gntime = unix_secs();

        let body = format!(
            "appid={}&exist=1&gntime={}&jlz={}&lang=us&lx=TXT&mx=0&ttl=120&xl=0&ym={}&zj={}",
            self.appid, gntime, encoded_value, ext_domain, url_encode(hostname)
        );

        let resp = post_api(&body, "/api/resolution/add", &self.appkey)?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Provider(format!("gname add JSON: {e}")))?;
        let code = v.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
        if code == 1 {
            return Ok(());
        }
        let msg = v.get("msg").and_then(|m| m.as_str()).unwrap_or("");
        if msg.contains("the same host records and record values") {
            return Ok(());
        }
        Err(Error::Provider(format!("gname add TXT: code={code} msg={msg}")))
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (ext_domain, ext_hostname) = resolve_zone(name)
            .map_err(|e| Error::Provider(format!("gname zone: {e}")))?;
        let hostname = if ext_hostname.is_empty() { "@" } else { &ext_hostname };

        let record_id = match find_record_id(&self.appid, &self.appkey, &ext_domain, hostname, value) {
            Ok(Some(id)) => id,
            Ok(None) => return Ok(()),
            Err(e) => {
                eprintln!("warning: gname cleanup lookup: {e}");
                return Ok(());
            }
        };

        let gntime = unix_secs();
        let body = format!(
            "appid={}&gntime={}&jxid={}&lang=us&ym={}",
            self.appid, gntime, record_id, ext_domain
        );
        let resp = post_api(&body, "/api/resolution/delete", &self.appkey)?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Provider(format!("gname del JSON: {e}")))?;
        let code = v.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
        if code != 1 {
            let msg = v.get("msg").and_then(|m| m.as_str()).unwrap_or("");
            eprintln!("warning: gname del TXT: code={code} msg={msg}");
        }
        Ok(())
    }
}

fn find_record_id(appid: &str, appkey: &str, ext_domain: &str, hostname: &str, value: &str) -> Result<Option<String>, Error> {
    let gntime = unix_secs();
    let body = format!(
        "appid={}&gntime={}&limit=1000&lx=TXT&page=1&ym={}",
        appid, gntime, ext_domain
    );
    let resp = post_api(&body, "/api/resolution/list", appkey)?;
    let v: Value = serde_json::from_str(&resp.body)
        .map_err(|e| Error::Provider(format!("gname list JSON: {e}")))?;

    let records = v.get("data").and_then(|d| d.as_array())
        .ok_or_else(|| Error::Provider("gname list: no data array".into()))?;

    for record in records {
        let zjt = record.get("zjt").and_then(|z| z.as_str()).unwrap_or("");
        let jxz = record.get("jxz").and_then(|j| j.as_str()).unwrap_or("");
        if zjt == hostname && jxz == value {
            if let Some(id) = record.get("id").and_then(|i| i.as_str()).map(|s| s.to_string()) {
                return Ok(Some(id));
            }
        }
    }
    Ok(None)
}

fn post_api(body: &str, path: &str, appkey: &str) -> Result<http::Response, Error> {
    let token = gntoken(body, appkey);
    let full_body = format!("{body}&gntoken={token}");
    let url = format!("{API_BASE}{path}");
    let resp = http::post(&url, full_body.as_bytes(), "application/x-www-form-urlencoded", &[])
        .map_err(|e| Error::Provider(format!("gname POST {path}: {e}")))?;
    if resp.status >= 400 {
        return Err(Error::Provider(format!("gname POST {path}: HTTP {} {}", resp.status, resp.body)));
    }
    Ok(resp)
}

fn gntoken(body: &str, appkey: &str) -> String {
    let data = format!("{body}{appkey}");
    md5_hex(data.as_bytes()).to_uppercase()
}

fn resolve_zone(fulldomain: &str) -> Result<(String, String), String> {
    let suffixes = fetch_tlds()?;
    let dot_count = fulldomain.chars().filter(|&c| c == '.').count();
    if dot_count == 0 {
        return Err(format!("invalid domain: {fulldomain}"));
    }
    if dot_count == 1 {
        return Ok((fulldomain.to_string(), String::new()));
    }

    let mut matched_suffix = String::new();
    for suffix in &suffixes {
        if fulldomain.ends_with(&format!(".{suffix}")) && suffix.len() > matched_suffix.len() {
            matched_suffix = suffix.clone();
        }
    }

    let ext_domain = if !matched_suffix.is_empty() {
        let prefix = &fulldomain[..fulldomain.len() - matched_suffix.len() - 1];
        let main_name = prefix.rsplit('.').next().unwrap_or(prefix);
        format!("{main_name}.{matched_suffix}")
    } else {
        let tld = fulldomain.rsplit('.').next().unwrap_or("");
        let tmp = &fulldomain[..fulldomain.len() - tld.len() - 1];
        let main = tmp.rsplit('.').next().unwrap_or(tmp);
        format!("{main}.{tld}")
    };

    if fulldomain == ext_domain {
        Ok((ext_domain, String::new()))
    } else {
        let hostname = &fulldomain[..fulldomain.len() - ext_domain.len() - 1];
        Ok((ext_domain, hostname.to_string()))
    }
}

fn fetch_tlds() -> Result<Vec<String>, String> {
    let resp = http::get(TLD_URL, &[])
        .map_err(|e| format!("fetch TLDs: {e}"))?;
    let v: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("TLD JSON: {e}"))?;
    if v.get("code").and_then(|c| c.as_i64()) != Some(1) {
        return Err("TLD API returned non-1 code".into());
    }
    let mut suffixes = Vec::new();
    if let Some(main) = v.get("main").and_then(|m| m.as_array()) {
        for s in main {
            if let Some(s) = s.as_str() {
                suffixes.push(s.to_string());
            }
        }
    }
    if let Some(sub) = v.get("sub").and_then(|s| s.as_array()) {
        for s in sub {
            if let Some(s) = s.as_str() {
                suffixes.push(s.to_string());
            }
        }
    }
    if suffixes.is_empty() {
        return Err("empty TLD list".into());
    }
    Ok(suffixes)
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

fn unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn md5_hex(input: &[u8]) -> String {
    let digest = md5(input);
    let mut hex = String::with_capacity(32);
    for b in digest {
        hex.push_str(&format!("{b:02x}"));
    }
    hex
}

fn md5(input: &[u8]) -> [u8; 16] {
    let s: [u32; 4] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476];
    let k: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
        0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
        0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
        0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
        0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
        0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
        0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
    ];
    let r: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
        5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
        4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
        6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];

    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut msg = input.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());

    let mut a0 = s[0];
    let mut b0 = s[1];
    let mut c0 = s[2];
    let mut d0 = s[3];

    for chunk in msg.chunks(64) {
        let mut m = [0u32; 16];
        for (i, word) in chunk.chunks(4).enumerate() {
            m[i] = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
        }

        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64u32 {
            let (f, g) = match i {
                0..=15 => ((b & c) | ((!b) & d), i as usize),
                16..=31 => ((d & b) | ((!d) & c), ((5 * i + 1) % 16) as usize),
                32..=47 => (b ^ c ^ d, ((3 * i + 5) % 16) as usize),
                _ => (c ^ (b | (!d)), ((7 * i) % 16) as usize),
            };
            let f = f.wrapping_add(a).wrapping_add(k[i as usize]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f.rotate_left(r[i as usize]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut result = [0u8; 16];
    result[0..4].copy_from_slice(&a0.to_le_bytes());
    result[4..8].copy_from_slice(&b0.to_le_bytes());
    result[8..12].copy_from_slice(&c0.to_le_bytes());
    result[12..16].copy_from_slice(&d0.to_le_bytes());
    result
}
