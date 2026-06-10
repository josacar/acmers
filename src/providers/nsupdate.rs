use std::collections::HashMap;
use std::net::UdpSocket;
use std::time::{SystemTime, UNIX_EPOCH};

use ring::hmac;

use crate::base64;
use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Nsupdate {
    servers: Vec<(String, u16)>,
    tsig_key: Option<TsigKey>,
    zone: Option<String>,
}

struct TsigKey {
    name: String,
    algorithm: TsigAlgorithm,
    secret: Vec<u8>,
}

#[derive(Clone, Copy)]
enum TsigAlgorithm {
    HmacMd5,
    HmacSha1,
    HmacSha256,
    HmacSha384,
    HmacSha512,
}

impl DnsProvider for Nsupdate {
    fn slug() -> &'static str {
        "nsupdate"
    }

    fn env_vars() -> &'static [&'static str] {
        &["NSUPDATE_SERVER", "NSUPDATE_SERVER_PORT", "NSUPDATE_KEY", "NSUPDATE_ZONE"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let server_str = env.get("NSUPDATE_SERVER")
            .map(|s| s.as_str())
            .unwrap_or("localhost");
        let port_str = env.get("NSUPDATE_SERVER_PORT")
            .map(|s| s.as_str())
            .unwrap_or("53");
        let port: u16 = port_str.parse()
            .map_err(|_| Error::Config("NSUPDATE_SERVER_PORT must be a number".into()))?;

        let servers: Vec<(String, u16)> = server_str
            .split(',')
            .map(|s| (s.trim().to_string(), port))
            .collect();

        let tsig_key = match env.get("NSUPDATE_KEY").filter(|s| !s.is_empty()) {
            Some(key_path) => Some(parse_tsig_key_file(key_path)?),
            None => None,
        };

        let zone = env.get("NSUPDATE_ZONE")
            .filter(|s| !s.is_empty())
            .map(|s| s.trim_end_matches('.').to_string());

        Ok(Box::new(Nsupdate { servers, tsig_key, zone }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let fqdn = ensure_fqdn(name);
        let zone = self.resolve_zone(&fqdn);
        let zone_name = ensure_fqdn(&zone);

        let mut msg = Vec::with_capacity(512);
        let header = build_update_header(0, 1, if self.tsig_key.is_some() { 1 } else { 0 });
        msg.extend_from_slice(&header);
        encode_name_into(&zone_name, &mut msg);
        msg.extend_from_slice(&[0x00, 0x06, 0x00, 0x01]);
        encode_name_into(&fqdn, &mut msg);
        msg.extend_from_slice(&[0x00, 0x10, 0x00, 0x01, 0x00, 0x00, 0x00, 0x3C]);
        let txt_bytes = value.as_bytes();
        msg.push((txt_bytes.len() + 1) as u8);
        msg.push(txt_bytes.len() as u8);
        msg.extend_from_slice(txt_bytes);

        let mut last_err = None;
        for (server, port) in &self.servers {
            match self.send_update(server, *port, &msg) {
                Ok(_) => return Ok(()),
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| Error::Provider("no servers configured".into())))
    }

    fn remove_txt(&self, _domain: &str, name: &str, _value: &str) -> ProviderResult {
        let fqdn = ensure_fqdn(name);
        let zone = self.resolve_zone(&fqdn);
        let zone_name = ensure_fqdn(&zone);

        let mut msg = Vec::with_capacity(512);
        let header = build_update_header(0, 1, if self.tsig_key.is_some() { 1 } else { 0 });
        msg.extend_from_slice(&header);
        encode_name_into(&zone_name, &mut msg);
        msg.extend_from_slice(&[0x00, 0x06, 0x00, 0x01]);
        encode_name_into(&fqdn, &mut msg);
        msg.extend_from_slice(&[0x00, 0x10, 0x00, 0xFF, 0x00, 0x00, 0x00, 0x00]);

        let mut last_err = None;
        for (server, port) in &self.servers {
            match self.send_update(server, *port, &msg) {
                Ok(_) => return Ok(()),
                Err(e) => last_err = Some(e),
            }
        }
        match last_err {
            Some(e) => {
                eprintln!("warning: cleanup failed: {e}");
                Ok(())
            }
            None => Ok(()),
        }
    }
}

impl Nsupdate {
    fn resolve_zone(&self, fqdn: &str) -> String {
        if let Some(ref z) = self.zone {
            return z.clone();
        }
        let parts: Vec<&str> = fqdn.trim_end_matches('.').split('.').collect();
        if parts.len() >= 2 {
            format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1])
        } else {
            fqdn.to_string()
        }
    }

    fn send_update(&self, server: &str, port: u16, msg: &[u8]) -> ProviderResult {
        let msg_id = u16::from_be_bytes([msg[0], msg[1]]);

        let msg = if let Some(ref key) = self.tsig_key {
            sign_message(msg, key)?
        } else {
            msg.to_vec()
        };

        let addr = format!("{server}:{port}");
        let response = send_udp(&addr, &msg)?;

        if response.len() < 12 {
            return Err(Error::Dns("response too short".into()));
        }

        let resp_id = u16::from_be_bytes([response[0], response[1]]);
        if resp_id != msg_id {
            return Err(Error::Dns(format!("response ID mismatch: sent {msg_id}, got {resp_id}")));
        }

        let rcode = u16::from_be_bytes([response[2], response[3]]) & 0x000F;
        if rcode != 0 {
            return Err(Error::Dns(format!("DNS UPDATE failed with rcode {rcode}")));
        }

        Ok(())
    }
}

fn build_update_header(zone_count: u16, update_count: u16, additional_count: u16) -> [u8; 12] {
    let id: u16 = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() & 0xFFFF) as u16;
    let mut hdr = [0u8; 12];
    hdr[0] = (id >> 8) as u8;
    hdr[1] = id as u8;
    hdr[2] = 0x28;
    hdr[3] = 0x00;
    let zc = zone_count.to_be_bytes();
    hdr[4] = zc[0];
    hdr[5] = zc[1];
    hdr[6] = 0;
    hdr[7] = 0;
    let uc = update_count.to_be_bytes();
    hdr[8] = uc[0];
    hdr[9] = uc[1];
    let ac = additional_count.to_be_bytes();
    hdr[10] = ac[0];
    hdr[11] = ac[1];
    hdr
}

fn encode_name_into(name: &str, buf: &mut Vec<u8>) {
    let name = name.trim_end_matches('.');
    if name.is_empty() {
        buf.push(0);
        return;
    }
    for label in name.split('.') {
        let bytes = label.as_bytes();
        buf.push(bytes.len() as u8);
        buf.extend_from_slice(bytes);
    }
    buf.push(0);
}

fn encode_name(name: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    encode_name_into(name, &mut buf);
    buf
}

fn sign_message(msg: &[u8], key: &TsigKey) -> Result<Vec<u8>, Error> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::Provider(format!("time: {e}")))?;
    let time_signed = now.as_secs();
    let fudge: u16 = 300;

    let alg_name = key.algorithm.domain_name();
    let alg_encoded = encode_name(&alg_name);
    let key_name_encoded = encode_name(&key.name);

    let time_bytes = time_to_48bit(time_signed);

    let mut mac_input = Vec::with_capacity(msg.len() + 128);
    mac_input.extend_from_slice(msg);
    mac_input.extend_from_slice(&key_name_encoded);
    mac_input.extend_from_slice(&(0x00FFu16).to_be_bytes());
    mac_input.extend_from_slice(&(0x00000000u32).to_be_bytes());
    mac_input.extend_from_slice(&alg_encoded);
    mac_input.extend_from_slice(&time_bytes);
    mac_input.extend_from_slice(&fudge.to_be_bytes());
    mac_input.extend_from_slice(&(0x0000u16).to_be_bytes());
    mac_input.extend_from_slice(&(0x0000u16).to_be_bytes());

    let mac = compute_hmac(key, &mac_input)?;
    let mac_len = mac.len() as u16;
    let orig_id = u16::from_be_bytes([msg[0], msg[1]]);

    let mut tsig_rdata = Vec::with_capacity(128);
    tsig_rdata.extend_from_slice(&alg_encoded);
    tsig_rdata.extend_from_slice(&time_bytes);
    tsig_rdata.extend_from_slice(&fudge.to_be_bytes());
    tsig_rdata.extend_from_slice(&mac_len.to_be_bytes());
    tsig_rdata.extend_from_slice(&mac);
    tsig_rdata.extend_from_slice(&orig_id.to_be_bytes());
    tsig_rdata.extend_from_slice(&(0x0000u16).to_be_bytes());
    tsig_rdata.extend_from_slice(&(0x0000u16).to_be_bytes());

    let mut result = Vec::with_capacity(msg.len() + key_name_encoded.len() + 10 + tsig_rdata.len());
    result.extend_from_slice(msg);
    result.extend_from_slice(&key_name_encoded);
    result.extend_from_slice(&(0x00FAu16).to_be_bytes());
    result.extend_from_slice(&(0x00FFu16).to_be_bytes());
    result.extend_from_slice(&(0x00000000u32).to_be_bytes());
    let rdlen = tsig_rdata.len() as u16;
    result.extend_from_slice(&rdlen.to_be_bytes());
    result.extend_from_slice(&tsig_rdata);

    let add_count = u16::from_be_bytes([result[10], result[11]]);
    let new_count = add_count + 1;
    result[10] = (new_count >> 8) as u8;
    result[11] = new_count as u8;

    Ok(result)
}

fn time_to_48bit(secs: u64) -> [u8; 6] {
    [
        ((secs >> 40) & 0xFF) as u8,
        ((secs >> 32) & 0xFF) as u8,
        ((secs >> 24) & 0xFF) as u8,
        ((secs >> 16) & 0xFF) as u8,
        ((secs >> 8) & 0xFF) as u8,
        (secs & 0xFF) as u8,
    ]
}

fn compute_hmac(key: &TsigKey, data: &[u8]) -> Result<Vec<u8>, Error> {
    let algorithm = match key.algorithm {
        TsigAlgorithm::HmacMd5 => hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
        TsigAlgorithm::HmacSha1 => hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
        TsigAlgorithm::HmacSha256 => hmac::HMAC_SHA256,
        TsigAlgorithm::HmacSha384 => hmac::HMAC_SHA384,
        TsigAlgorithm::HmacSha512 => hmac::HMAC_SHA512,
    };
    let signing_key = hmac::Key::new(algorithm, &key.secret);
    let tag = hmac::sign(&signing_key, data);
    Ok(tag.as_ref().to_vec())
}

fn send_udp(addr: &str, msg: &[u8]) -> Result<Vec<u8>, Error> {
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| Error::Dns(format!("bind UDP: {e}")))?;
    socket.set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .map_err(|e| Error::Dns(format!("set timeout: {e}")))?;
    socket.send_to(msg, addr)
        .map_err(|e| Error::Dns(format!("send to {addr}: {e}")))?;
    let mut buf = vec![0u8; 4096];
    let n = socket.recv_from(&mut buf)
        .map_err(|e| Error::Dns(format!("recv from {addr}: {e}")))?;
    buf.truncate(n.0);
    Ok(buf)
}

fn parse_tsig_key_file(path: &str) -> Result<TsigKey, Error> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| Error::Config(format!("read TSIG key file '{path}': {e}")))?;
    parse_tsig_key_content(&content)
}

fn parse_tsig_key_content(content: &str) -> Result<TsigKey, Error> {
    let cleaned: String = content
        .lines()
        .map(|l| {
            let l = l.split("//").next().unwrap_or(l);
            let l = l.split('#').next().unwrap_or(l);
            l
        })
        .collect::<Vec<_>>()
        .join("\n");

    let name = extract_quoted_string(&cleaned, "key")
        .ok_or_else(|| Error::Config("TSIG key file: missing key name".into()))?;

    let alg_str = extract_value(&cleaned, "algorithm")
        .ok_or_else(|| Error::Config("TSIG key file: missing algorithm".into()))?;

    let secret_b64 = extract_quoted_string(&cleaned, "secret")
        .ok_or_else(|| Error::Config("TSIG key file: missing secret".into()))?;

    let algorithm = match alg_str.to_lowercase().replace('-', "").as_str() {
        "hmacmd5" => TsigAlgorithm::HmacMd5,
        "hmacsha1" => TsigAlgorithm::HmacSha1,
        "hmacsha256" => TsigAlgorithm::HmacSha256,
        "hmacsha384" => TsigAlgorithm::HmacSha384,
        "hmacsha512" => TsigAlgorithm::HmacSha512,
        other => return Err(Error::Config(format!("unsupported TSIG algorithm: {other}"))),
    };

    let secret = base64::decode_std(&secret_b64)
        .map_err(|e| Error::Config(format!("decode TSIG secret base64: {e}")))?;

    Ok(TsigKey {
        name: ensure_fqdn(&name),
        algorithm,
        secret,
    })
}

fn extract_value(content: &str, keyword: &str) -> Option<String> {
    let lower = content.to_lowercase();
    let kw_lower = keyword.to_lowercase();
    let pos = lower.find(&kw_lower)?;
    let after = &content[pos + kw_lower.len()..];
    let after = after.trim_start();
    if after.starts_with('"') {
        let rest = &after[1..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    } else {
        let end = after.find(|c: char| c == ';' || c == '\n' || c == '}' || c.is_whitespace())
            .unwrap_or(after.len());
        let val = after[..end].trim();
        if val.is_empty() { None } else { Some(val.to_string()) }
    }
}

fn extract_quoted_string(content: &str, keyword: &str) -> Option<String> {
    let lower = content.to_lowercase();
    let kw_lower = keyword.to_lowercase();
    let pos = lower.find(&kw_lower)?;
    let after = &content[pos + kw_lower.len()..];
    let first_quote = after.find('"')?;
    let rest = &after[first_quote + 1..];
    let second_quote = rest.find('"')?;
    Some(rest[..second_quote].to_string())
}

fn ensure_fqdn(name: &str) -> String {
    if name.ends_with('.') {
        name.to_string()
    } else {
        format!("{name}.")
    }
}

impl TsigAlgorithm {
    fn domain_name(&self) -> &'static str {
        match self {
            TsigAlgorithm::HmacMd5 => "hmac-md5.sig-alg.reg.int.",
            TsigAlgorithm::HmacSha1 => "hmac-sha1.",
            TsigAlgorithm::HmacSha256 => "hmac-sha256.",
            TsigAlgorithm::HmacSha384 => "hmac-sha384.",
            TsigAlgorithm::HmacSha512 => "hmac-sha512.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_name() {
        let encoded = encode_name("example.com.");
        assert_eq!(encoded, vec![7, b'e', b'x', b'a', b'm', b'p', b'l', b'e', 3, b'c', b'o', b'm', 0]);
    }

    #[test]
    fn test_encode_name_no_trailing_dot() {
        let encoded = encode_name("example.com");
        assert_eq!(encoded, vec![7, b'e', b'x', b'a', b'm', b'p', b'l', b'e', 3, b'c', b'o', b'm', 0]);
    }

    #[test]
    fn test_ensure_fqdn() {
        assert_eq!(ensure_fqdn("example.com"), "example.com.");
        assert_eq!(ensure_fqdn("example.com."), "example.com.");
    }

    #[test]
    fn test_parse_tsig_key_md5() {
        let content = r#"
key "mykey" {
    algorithm hmac-md5;
    secret "c2VjcmV0a2V5";
};
"#;
        let key = parse_tsig_key_content(content).unwrap();
        assert_eq!(key.name, "mykey.");
        assert_eq!(key.secret, b"secretkey");
    }

    #[test]
    fn test_parse_tsig_key_sha256() {
        let content = r#"
key "tsig-key.example.com" {
    algorithm hmac-sha256;
    secret "dGVzdHNlY3JldA==";
};
"#;
        let key = parse_tsig_key_content(content).unwrap();
        assert_eq!(key.name, "tsig-key.example.com.");
        assert_eq!(key.secret, b"testsecret");
    }

    #[test]
    fn test_parse_tsig_key_with_comments() {
        let content = r#"
// This is a comment
key "testkey" {
    algorithm hmac-sha256; # inline comment
    secret "aGVsbG8=";
};
"#;
        let key = parse_tsig_key_content(content).unwrap();
        assert_eq!(key.name, "testkey.");
        assert_eq!(key.secret, b"hello");
    }

    #[test]
    fn test_build_update_header() {
        let hdr = build_update_header(1, 1, 0);
        assert_eq!(hdr[2] & 0xF0, 0x20);
        let zc = u16::from_be_bytes([hdr[4], hdr[5]]);
        assert_eq!(zc, 1);
        let uc = u16::from_be_bytes([hdr[8], hdr[9]]);
        assert_eq!(uc, 1);
        let ac = u16::from_be_bytes([hdr[10], hdr[11]]);
        assert_eq!(ac, 0);
    }

    #[test]
    fn test_extract_quoted_string() {
        let s = r#"key "myname" { }"#;
        assert_eq!(extract_quoted_string(s, "key"), Some("myname".to_string()));
    }

    #[test]
    fn test_resolve_zone_default() {
        let ns = Nsupdate {
            servers: vec![("localhost".into(), 53)],
            tsig_key: None,
            zone: None,
        };
        assert_eq!(ns.resolve_zone("_acme-challenge.example.com."), "example.com");
    }

    #[test]
    fn test_resolve_zone_override() {
        let ns = Nsupdate {
            servers: vec![("localhost".into(), 53)],
            tsig_key: None,
            zone: Some("myzone.com".into()),
        };
        assert_eq!(ns.resolve_zone("_acme-challenge.sub.myzone.com."), "myzone.com");
    }

    #[test]
    fn test_tsig_algorithm_domain_names() {
        assert_eq!(TsigAlgorithm::HmacMd5.domain_name(), "hmac-md5.sig-alg.reg.int.");
        assert_eq!(TsigAlgorithm::HmacSha256.domain_name(), "hmac-sha256.");
        assert_eq!(TsigAlgorithm::HmacSha512.domain_name(), "hmac-sha512.");
    }

    #[test]
    fn test_compute_hmac_sha256() {
        let key = TsigKey {
            name: "test.".into(),
            algorithm: TsigAlgorithm::HmacSha256,
            secret: b"secret".to_vec(),
        };
        let mac = compute_hmac(&key, b"hello").unwrap();
        assert!(!mac.is_empty());
    }

    #[test]
    fn test_sign_message_produces_valid_structure() {
        let key = TsigKey {
            name: "testkey.".into(),
            algorithm: TsigAlgorithm::HmacSha256,
            secret: b"testsecret".to_vec(),
        };
        let mut msg = Vec::new();
        let hdr = build_update_header(1, 1, 0);
        msg.extend_from_slice(&hdr);
        encode_name_into("example.com.", &mut msg);
        msg.extend_from_slice(&[0x00, 0x06, 0x00, 0x01]);
        encode_name_into("_acme-challenge.example.com.", &mut msg);
        msg.extend_from_slice(&[0x00, 0x10, 0x00, 0x01, 0x00, 0x00, 0x00, 0x3C]);
        msg.push(5);
        msg.push(4);
        msg.extend_from_slice(b"test");

        let signed = sign_message(&msg, &key).unwrap();
        assert!(signed.len() > msg.len());
        let ac = u16::from_be_bytes([signed[10], signed[11]]);
        assert_eq!(ac, 1);
    }

    #[test]
    fn test_nsupdate_new_no_key() {
        let p = Nsupdate {
            servers: vec![("192.168.1.1".into(), 53)],
            tsig_key: None,
            zone: None,
        };
        assert_eq!(p.servers.len(), 1);
        assert_eq!(p.servers[0].0, "192.168.1.1");
        assert_eq!(p.servers[0].1, 53);
        assert!(p.tsig_key.is_none());
    }

    #[test]
    fn test_nsupdate_new_multi_server() {
        let p = Nsupdate {
            servers: vec![("ns1.example.com".into(), 5353), ("ns2.example.com".into(), 5353)],
            tsig_key: None,
            zone: None,
        };
        assert_eq!(p.servers.len(), 2);
        assert_eq!(p.servers[0].1, 5353);
        assert_eq!(p.servers[1].0, "ns2.example.com");
    }

    #[test]
    fn test_nsupdate_new_with_zone() {
        let p = Nsupdate {
            servers: vec![("localhost".into(), 53)],
            tsig_key: None,
            zone: Some("example.com".into()),
        };
        assert_eq!(p.zone, Some("example.com".into()));
    }

    #[test]
    fn test_full_add_message_build() {
        let mut msg = Vec::new();
        let hdr = build_update_header(0, 1, 0);
        msg.extend_from_slice(&hdr);
        encode_name_into("example.com.", &mut msg);
        msg.extend_from_slice(&[0x00, 0x06, 0x00, 0x01]);
        encode_name_into("_acme-challenge.www.example.com.", &mut msg);
        msg.extend_from_slice(&[0x00, 0x10, 0x00, 0x01]);
        msg.extend_from_slice(&[0x00, 0x00, 0x00, 0x3C]);
        let value = "XKrxpRBosdIKFzxW_CT3KLZNf6q0HG9i01zxXp5CPBs";
        let txt_bytes = value.as_bytes();
        msg.push((txt_bytes.len() + 1) as u8);
        msg.push(txt_bytes.len() as u8);
        msg.extend_from_slice(txt_bytes);

        assert!(msg.len() > 12);
        let opcode = (msg[2] >> 3) & 0x0F;
        assert_eq!(opcode, 5);
    }

    #[test]
    fn test_full_remove_message_build() {
        let mut msg = Vec::new();
        let hdr = build_update_header(0, 1, 0);
        msg.extend_from_slice(&hdr);
        encode_name_into("example.com.", &mut msg);
        msg.extend_from_slice(&[0x00, 0x06, 0x00, 0x01]);
        encode_name_into("_acme-challenge.www.example.com.", &mut msg);
        msg.extend_from_slice(&[0x00, 0x10, 0x00, 0xFF, 0x00, 0x00, 0x00, 0x00]);

        assert!(msg.len() > 12);
        let opcode = (msg[2] >> 3) & 0x0F;
        assert_eq!(opcode, 5);
    }

    #[test]
    fn test_udp_mock_server() {
        use std::net::UdpSocket as StdUdpSocket;
        use std::sync::mpsc;
        use std::thread;

        let server_socket = StdUdpSocket::bind("127.0.0.1:0").unwrap();
        server_socket.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let server_addr = server_socket.local_addr().unwrap();
        let port = server_addr.port();

        let (tx, rx) = mpsc::channel::<()>();
        let handle = thread::spawn(move || {
            let mut buf = [0u8; 4096];
            if let Ok((n, src)) = server_socket.recv_from(&mut buf) {
                let req = &buf[..n];
                let mut resp = vec![0u8; 12];
                resp[0] = req[0];
                resp[1] = req[1];
                resp[2] = 0x80;
                resp[3] = 0x00;
                let _ = server_socket.send_to(&resp, src);
            }
            let _ = rx.recv();
        });

        let ns = Nsupdate {
            servers: vec![("127.0.0.1".into(), port)],
            tsig_key: None,
            zone: Some("example.com".into()),
        };

        let result = ns.add_txt("example.com", "_acme-challenge.example.com", "testvalue");
        assert!(result.is_ok());

        drop(tx);
        let _ = handle.join();
    }

    #[test]
    fn test_udp_mock_server_remove() {
        use std::net::UdpSocket as StdUdpSocket;
        use std::sync::mpsc;
        use std::thread;

        let server_socket = StdUdpSocket::bind("127.0.0.1:0").unwrap();
        server_socket.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let port = server_socket.local_addr().unwrap().port();

        let (tx, rx) = mpsc::channel::<()>();
        let handle = thread::spawn(move || {
            let mut buf = [0u8; 4096];
            if let Ok((n, src)) = server_socket.recv_from(&mut buf) {
                let req = &buf[..n];
                let mut resp = vec![0u8; 12];
                resp[0] = req[0];
                resp[1] = req[1];
                resp[2] = 0x80;
                resp[3] = 0x00;
                let _ = server_socket.send_to(&resp, src);
            }
            let _ = rx.recv();
        });

        let ns = Nsupdate {
            servers: vec![("127.0.0.1".into(), port)],
            tsig_key: None,
            zone: Some("example.com".into()),
        };

        let result = ns.remove_txt("example.com", "_acme-challenge.example.com", "testvalue");
        assert!(result.is_ok());

        drop(tx);
        let _ = handle.join();
    }

    #[test]
    fn test_udp_mock_server_error_response() {
        use std::net::UdpSocket as StdUdpSocket;
        use std::sync::mpsc;
        use std::thread;

        let server_socket = StdUdpSocket::bind("127.0.0.1:0").unwrap();
        server_socket.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let port = server_socket.local_addr().unwrap().port();

        let (tx, rx) = mpsc::channel::<()>();
        let handle = thread::spawn(move || {
            let mut buf = [0u8; 4096];
            if let Ok((n, src)) = server_socket.recv_from(&mut buf) {
                let req = &buf[..n];
                let mut resp = vec![0u8; 12];
                resp[0] = req[0];
                resp[1] = req[1];
                resp[2] = 0x80;
                resp[3] = 0x05;
                let _ = server_socket.send_to(&resp, src);
            }
            let _ = rx.recv();
        });

        let ns = Nsupdate {
            servers: vec![("127.0.0.1".into(), port)],
            tsig_key: None,
            zone: Some("example.com".into()),
        };

        let result = ns.add_txt("example.com", "_acme-challenge.example.com", "testvalue");
        assert!(result.is_err());

        drop(tx);
        let _ = handle.join();
    }

    #[test]
    fn test_remove_txt_idempotent_on_error() {
        let ns = Nsupdate {
            servers: vec![("192.0.2.1".into(), 59999)],
            tsig_key: None,
            zone: Some("example.com".into()),
        };
        let result = ns.remove_txt("example.com", "_acme-challenge.example.com", "testvalue");
        assert!(result.is_ok());
    }
}
