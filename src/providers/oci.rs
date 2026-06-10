use std::collections::HashMap;

use ring::digest::{digest, SHA256};
use ring::signature::{RsaKeyPair, RSA_PKCS1_SHA256, KeyPair as RingKeyPair};
use ring::rand::SystemRandom;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Oci {
    tenancy: String,
    user: String,
    region: String,
    key_pair: RsaKeyPair,
    fingerprint: String,
}

impl DnsProvider for Oci {
    fn slug() -> &'static str {
        "oci"
    }

    fn env_vars() -> &'static [&'static str] {
        &["OCI_PRIVKEY", "OCI_TENANCY", "OCI_USER", "OCI_REGION"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let config_file = env.get("OCI_CLI_CONFIG_FILE")
            .cloned()
            .unwrap_or_else(|| {
                let home = std::env::var("HOME").unwrap_or_default();
                format!("{home}/.oci/config")
            });
        let profile = env.get("OCI_CLI_PROFILE")
            .cloned()
            .unwrap_or_else(|| "DEFAULT".to_string());

        let ini = if std::path::Path::new(&config_file).exists() {
            std::fs::read_to_string(&config_file)
                .map_err(|e| Error::Config(format!("read OCI config: {e}")))?
        } else {
            String::new()
        };

        let tenancy = resolve_env_or_ini(env, &ini, &profile, "OCI_CLI_TENANCY", "OCI_TENANCY", "tenancy")?;
        let user = resolve_env_or_ini(env, &ini, &profile, "OCI_CLI_USER", "OCI_USER", "user")?;
        let region = resolve_env_or_ini(env, &ini, &profile, "OCI_CLI_REGION", "OCI_REGION", "region")?;

        let pem = if let Some(k) = env.get("OCI_PRIVKEY") {
            if k.contains("BEGIN") {
                k.clone()
            } else if std::path::Path::new(k).exists() {
                std::fs::read_to_string(k)
                    .map_err(|e| Error::Config(format!("read OCI key file: {e}")))?
            } else {
                let decoded = base64::decode_std(k)
                    .map_err(|e| Error::Config(format!("decode OCI_PRIVKEY: {e}")))?;
                String::from_utf8(decoded)
                    .map_err(|e| Error::Config(format!("OCI_PRIVKEY not valid UTF-8: {e}")))?
            }
        } else if let Some(key_file) = read_ini(&ini, &profile, "key_file") {
            if std::path::Path::new(&key_file).exists() {
                std::fs::read_to_string(&key_file)
                    .map_err(|e| Error::Config(format!("read OCI key_file: {e}")))?
            } else {
                return Err(Error::Config(format!("OCI key_file not found: {key_file}")));
            }
        } else {
            return Err(Error::Config("OCI_PRIVKEY or key_file in config required".into()));
        };

        let der = pem_to_der(&pem)?;
        let key_pair = RsaKeyPair::from_pkcs8(&der)
            .map_err(|e| Error::Crypto(format!("parse OCI RSA key: {e}")))?;
        let fingerprint = compute_fingerprint(&key_pair);

        Ok(Box::new(Oci { tenancy, user, region, key_pair, fingerprint }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone, sub_domain) = self.find_zone(name)?;
        let fqdn = format!("{sub_domain}.{zone}");
        let body = serde_json::json!({
            "items": [{
                "domain": fqdn,
                "rdata": value,
                "rtype": "TXT",
                "ttl": 30,
                "operation": "ADD"
            }]
        });
        let body_str = serde_json::to_string(&body).unwrap();
        let path = format!("/20180115/zones/{zone}/records");
        let resp = self.signed_request("PATCH", &path, Some(body_str.as_bytes()))?;
        if resp.status >= 300 {
            return Err(Error::Provider(format!("OCI add TXT: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone, sub_domain) = match self.find_zone(name) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        let fqdn = format!("{sub_domain}.{zone}");
        let body = serde_json::json!({
            "items": [{
                "domain": fqdn,
                "rdata": value,
                "rtype": "TXT",
                "operation": "REMOVE"
            }]
        });
        let body_str = serde_json::to_string(&body).unwrap();
        let path = format!("/20180115/zones/{zone}/records");
        let _ = self.signed_request("PATCH", &path, Some(body_str.as_bytes()));
        Ok(())
    }
}

impl Oci {
    fn find_zone(&self, fqdn: &str) -> Result<(String, String), Error> {
        let parts: Vec<&str> = fqdn.split('.').collect();
        for i in 0..parts.len().saturating_sub(1) {
            let candidate = parts[i..].join(".");
            let path = format!("/20180115/zones/{candidate}");
            match self.signed_request("GET", &path, None) {
                Ok(resp) if resp.status < 300 => {
                    let sub_domain = parts[..i].join(".");
                    return Ok((candidate, sub_domain));
                }
                _ => continue,
            }
        }
        Err(Error::Provider(format!("zone not found for {fqdn}")))
    }

    fn signed_request(&self, method: &str, target: &str, body: Option<&[u8]>) -> Result<http::Response, Error> {
        let host = format!("dns.{}.oraclecloud.com", self.region);
        let key_id = format!("{}/{}/{}", self.tenancy, self.user, self.fingerprint);
        let date = rfc1123_date();
        let method_lower = method.to_lowercase();

        let mut string_to_sign = format!(
            "(request-target): {} {}\ndate: {}\nhost: {}",
            method_lower, target, date, host
        );
        let mut sig_headers = "(request-target) date host".to_string();

        let mut extra_headers: Vec<(String, String)> = Vec::new();

        if let Some(body_bytes) = body {
            let body_sha256 = base64::encode_std(digest(&SHA256, body_bytes).as_ref());
            let content_type = "application/json";
            let content_length = body_bytes.len();
            string_to_sign.push_str(&format!(
                "\nx-content-sha256: {}\ncontent-type: {}\ncontent-length: {}",
                body_sha256, content_type, content_length
            ));
            sig_headers.push_str(" x-content-sha256 content-type content-length");
            extra_headers.push(("x-content-sha256".to_string(), body_sha256));
            extra_headers.push(("content-type".to_string(), content_type.to_string()));
            extra_headers.push(("content-length".to_string(), content_length.to_string()));
        }

        let signature = rsa_sign(&self.key_pair, string_to_sign.as_bytes())?;
        let auth = format!(
            r#"Signature version="1",keyId="{}",algorithm="rsa-sha256",headers="{}",signature="{}""#,
            key_id, sig_headers, signature
        );

        let url = format!("https://{}{}", host, target);

        let mut headers: Vec<(&str, &str)> = vec![
            ("Date", &date),
            ("Host", &host),
            ("Authorization", &auth),
        ];

        let extra_refs: Vec<(&str, &str)> = extra_headers.iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        headers.extend(extra_refs);

        match method {
            "GET" => http::get(&url, &headers)
                .map_err(|e| Error::Provider(format!("OCI GET: {e}"))),
            "PATCH" => {
                let body_bytes = body.unwrap_or(&[]);
                http::patch(&url, body_bytes, "application/json", &headers)
                    .map_err(|e| Error::Provider(format!("OCI PATCH: {e}")))
            }
            _ => Err(Error::Provider(format!("unsupported method: {method}"))),
        }
    }
}

fn resolve_env_or_ini(
    env: &HashMap<String, String>,
    ini: &str,
    profile: &str,
    cli_var: &str,
    short_var: &str,
    ini_key: &str,
) -> Result<String, Error> {
    if let Some(v) = env.get(short_var).filter(|s| !s.is_empty()) {
        return Ok(v.clone());
    }
    if let Some(v) = env.get(cli_var).filter(|s| !s.is_empty()) {
        return Ok(v.clone());
    }
    if let Some(v) = read_ini(ini, profile, ini_key) {
        if !v.is_empty() {
            return Ok(v);
        }
    }
    Err(Error::Config(format!("{} or {} required", short_var, cli_var)))
}

fn read_ini(content: &str, section: &str, key: &str) -> Option<String> {
    let section_header = format!("[{section}]");
    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_section = trimmed.eq_ignore_ascii_case(&section_header);
            continue;
        }
        if in_section {
            if let Some((k, v)) = trimmed.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

fn pem_to_der(pem: &str) -> Result<Vec<u8>, Error> {
    let b64: String = pem.lines()
        .filter(|l| !l.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");
    base64::decode_std(&b64)
        .map_err(|e| Error::Crypto(format!("decode PEM: {e}")).into())
}

fn compute_fingerprint(key_pair: &RsaKeyPair) -> String {
    let pub_der = key_pair.public_key().as_ref();
    let md5 = md5_hash(pub_der);
    md5.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(":")
}

fn rsa_sign(key_pair: &RsaKeyPair, data: &[u8]) -> Result<String, Error> {
    let rng = SystemRandom::new();
    let mut sig = vec![0u8; key_pair.public().modulus_len()];
    key_pair.sign(&RSA_PKCS1_SHA256, &rng, data, &mut sig)
        .map_err(|e| Error::Crypto(format!("RSA sign: {e}")))?;
    Ok(base64::encode_std(&sig))
}

fn rfc1123_date() -> String {
    let now = time::OffsetDateTime::now_utc();
    let weekday = match now.weekday() {
        time::Weekday::Monday => "Mon",
        time::Weekday::Tuesday => "Tue",
        time::Weekday::Wednesday => "Wed",
        time::Weekday::Thursday => "Thu",
        time::Weekday::Friday => "Fri",
        time::Weekday::Saturday => "Sat",
        time::Weekday::Sunday => "Sun",
    };
    let month = match now.month() {
        time::Month::January => "Jan",
        time::Month::February => "Feb",
        time::Month::March => "Mar",
        time::Month::April => "Apr",
        time::Month::May => "May",
        time::Month::June => "Jun",
        time::Month::July => "Jul",
        time::Month::August => "Aug",
        time::Month::September => "Sep",
        time::Month::October => "Oct",
        time::Month::November => "Nov",
        time::Month::December => "Dec",
    };
    format!(
        "{}, {:02} {} {:04} {:02}:{:02}:{:02} GMT",
        weekday, now.day(), month, now.year(), now.hour(), now.minute(), now.second()
    )
}

fn md5_hash(data: &[u8]) -> [u8; 16] {
    let mut state: [u32; 4] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476];
    let bit_len = (data.len() as u64) * 8;
    let mut buf = Vec::with_capacity(data.len() + 72);
    buf.extend_from_slice(data);
    buf.push(0x80);
    while buf.len() % 64 != 56 {
        buf.push(0);
    }
    buf.extend_from_slice(&bit_len.to_le_bytes());

    for chunk in buf.chunks_exact(64) {
        let mut block = [0u8; 64];
        block.copy_from_slice(chunk);
        md5_transform(&mut state, &block);
    }

    let mut result = [0u8; 16];
    for (i, &s) in state.iter().enumerate() {
        result[i * 4..i * 4 + 4].copy_from_slice(&s.to_le_bytes());
    }
    result
}

fn md5_transform(state: &mut [u32; 4], block: &[u8; 64]) {
    let mut x = [0u32; 16];
        for i in 0..16 {
            x[i] = u32::from_le_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }

        let (mut a, mut b, mut c, mut d) = (state[0], state[1], state[2], state[3]);

        const S11: u32 = 7;  const S12: u32 = 12; const S13: u32 = 17; const S14: u32 = 22;
        const S21: u32 = 5;  const S22: u32 = 9;  const S23: u32 = 14; const S24: u32 = 20;
        const S31: u32 = 4;  const S32: u32 = 11; const S33: u32 = 16; const S34: u32 = 23;
        const S41: u32 = 6;  const S42: u32 = 10; const S43: u32 = 15; const S44: u32 = 21;

        macro_rules! ff { ($a:expr, $b:expr, $c:expr, $d:expr, $x:expr, $s:expr, $ac:expr) => {
            $a = $a.wrapping_add((($b & $c) | (!$b & $d))).wrapping_add($x).wrapping_add($ac);
            $a = $b.wrapping_add($a.rotate_left($s));
        }}
        macro_rules! gg { ($a:expr, $b:expr, $c:expr, $d:expr, $x:expr, $s:expr, $ac:expr) => {
            $a = $a.wrapping_add((($b & $d) | ($c & !$d))).wrapping_add($x).wrapping_add($ac);
            $a = $b.wrapping_add($a.rotate_left($s));
        }}
        macro_rules! hh { ($a:expr, $b:expr, $c:expr, $d:expr, $x:expr, $s:expr, $ac:expr) => {
            $a = $a.wrapping_add($b ^ $c ^ $d).wrapping_add($x).wrapping_add($ac);
            $a = $b.wrapping_add($a.rotate_left($s));
        }}
        macro_rules! ii { ($a:expr, $b:expr, $c:expr, $d:expr, $x:expr, $s:expr, $ac:expr) => {
            $a = $a.wrapping_add($c ^ ($b | !$d)).wrapping_add($x).wrapping_add($ac);
            $a = $b.wrapping_add($a.rotate_left($s));
        }}

        ff!(a, b, c, d, x[ 0], S11, 0xd76aa478);
        ff!(d, a, b, c, x[ 1], S12, 0xe8c7b756);
        ff!(c, d, a, b, x[ 2], S13, 0x242070db);
        ff!(b, c, d, a, x[ 3], S14, 0xc1bdceee);
        ff!(a, b, c, d, x[ 4], S11, 0xf57c0faf);
        ff!(d, a, b, c, x[ 5], S12, 0x4787c62a);
        ff!(c, d, a, b, x[ 6], S13, 0xa8304613);
        ff!(b, c, d, a, x[ 7], S14, 0xfd469501);
        ff!(a, b, c, d, x[ 8], S11, 0x698098d8);
        ff!(d, a, b, c, x[ 9], S12, 0x8b44f7af);
        ff!(c, d, a, b, x[10], S13, 0xffff5bb1);
        ff!(b, c, d, a, x[11], S14, 0x895cd7be);
        ff!(a, b, c, d, x[12], S11, 0x6b901122);
        ff!(d, a, b, c, x[13], S12, 0xfd987193);
        ff!(c, d, a, b, x[14], S13, 0xa679438e);
        ff!(b, c, d, a, x[15], S14, 0x49b40821);

        gg!(a, b, c, d, x[ 1], S21, 0xf61e2562);
        gg!(d, a, b, c, x[ 6], S22, 0xc040b340);
        gg!(c, d, a, b, x[11], S23, 0x265e5a51);
        gg!(b, c, d, a, x[ 0], S24, 0xe9b6c7aa);
        gg!(a, b, c, d, x[ 5], S21, 0xd62f105d);
        gg!(d, a, b, c, x[10], S22, 0x02441453);
        gg!(c, d, a, b, x[15], S23, 0xd8a1e681);
        gg!(b, c, d, a, x[ 4], S24, 0xe7d3fbc8);
        gg!(a, b, c, d, x[ 9], S21, 0x21e1cde6);
        gg!(d, a, b, c, x[14], S22, 0xc33707d6);
        gg!(c, d, a, b, x[ 3], S23, 0xf4d50d87);
        gg!(b, c, d, a, x[ 8], S24, 0x455a14ed);
        gg!(a, b, c, d, x[13], S21, 0xa9e3e905);
        gg!(d, a, b, c, x[ 2], S22, 0xfcefa3f8);
        gg!(c, d, a, b, x[ 7], S23, 0x676f02d9);
        gg!(b, c, d, a, x[12], S24, 0x8d2a4c8a);

        hh!(a, b, c, d, x[ 5], S31, 0xfffa3942);
        hh!(d, a, b, c, x[ 8], S32, 0x8771f681);
        hh!(c, d, a, b, x[11], S33, 0x6d9d6122);
        hh!(b, c, d, a, x[14], S34, 0xfde5380c);
        hh!(a, b, c, d, x[ 1], S31, 0xa4beea44);
        hh!(d, a, b, c, x[ 4], S32, 0x4bdecfa9);
        hh!(c, d, a, b, x[ 7], S33, 0xf6bb4b60);
        hh!(b, c, d, a, x[10], S34, 0xbebfbc70);
        hh!(a, b, c, d, x[13], S31, 0x289b7ec6);
        hh!(d, a, b, c, x[ 0], S32, 0xeaa127fa);
        hh!(c, d, a, b, x[ 3], S33, 0xd4ef3085);
        hh!(b, c, d, a, x[ 6], S34, 0x04881d05);
        hh!(a, b, c, d, x[ 9], S31, 0xd9d4d039);
        hh!(d, a, b, c, x[12], S32, 0xe6db99e5);
        hh!(c, d, a, b, x[15], S33, 0x1fa27cf8);
        hh!(b, c, d, a, x[ 2], S34, 0xc4ac5665);

        ii!(a, b, c, d, x[ 0], S41, 0xf4292244);
        ii!(d, a, b, c, x[ 7], S42, 0x432aff97);
        ii!(c, d, a, b, x[14], S43, 0xab9423a7);
        ii!(b, c, d, a, x[ 5], S44, 0xfc93a039);
        ii!(a, b, c, d, x[12], S41, 0x655b59c3);
        ii!(d, a, b, c, x[ 3], S42, 0x8f0ccc92);
        ii!(c, d, a, b, x[10], S43, 0xffeff47d);
        ii!(b, c, d, a, x[ 1], S44, 0x85845dd1);
        ii!(a, b, c, d, x[ 8], S41, 0x6fa87e4f);
        ii!(d, a, b, c, x[15], S42, 0xfe2ce6e0);
        ii!(c, d, a, b, x[ 6], S43, 0xa3014314);
        ii!(b, c, d, a, x[13], S44, 0x4e0811a1);
        ii!(a, b, c, d, x[ 4], S41, 0xf7537e82);
        ii!(d, a, b, c, x[11], S42, 0xbd3af235);
        ii!(c, d, a, b, x[ 2], S43, 0x2ad7d2bb);
        ii!(b, c, d, a, x[ 9], S44, 0xeb86d391);

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_md5_empty() {
        let result = md5_hash(b"");
        assert_eq!(base64::hex(&result), "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn test_md5_abc() {
        let result = md5_hash(b"abc");
        assert_eq!(base64::hex(&result), "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn test_md5_longer() {
        let result = md5_hash(b"message digest");
        assert_eq!(base64::hex(&result), "f96b697d7cb7938d525a2f31aaf161d0");
    }

    #[test]
    fn test_read_ini() {
        let content = "[DEFAULT]\ntenancy=ocid1.tenancy.oc1..abc\nuser=ocid1.user.oc1..def\nregion=us-phoenix-1\nkey_file=/home/user/.oci/key.pem\n\n[OTHER]\ntenancy=ocid1.tenancy.oc1..xyz\n";
        assert_eq!(read_ini(content, "DEFAULT", "tenancy").unwrap(), "ocid1.tenancy.oc1..abc");
        assert_eq!(read_ini(content, "DEFAULT", "region").unwrap(), "us-phoenix-1");
        assert_eq!(read_ini(content, "OTHER", "tenancy").unwrap(), "ocid1.tenancy.oc1..xyz");
        assert!(read_ini(content, "OTHER", "user").is_none());
        assert!(read_ini(content, "MISSING", "tenancy").is_none());
    }

    #[test]
    fn test_rfc1123_date_format() {
        let d = rfc1123_date();
        assert!(d.ends_with("GMT"));
        assert!(d.contains(","));
    }

    #[test]
    fn test_pem_to_der() {
        let pem = "-----BEGIN PRIVATE KEY-----\nMIIB\n-----END PRIVATE KEY-----";
        let der = pem_to_der(pem).unwrap();
        assert_eq!(der, base64::decode_std("MIIB").unwrap());
    }
}
