use std::collections::HashMap;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Efficientip {
    server: String,
    creds: Option<String>,
    token_key: Option<String>,
    token_secret: Option<String>,
    dns_name: Option<String>,
    dns_view: Option<String>,
}

impl DnsProvider for Efficientip {
    fn slug() -> &'static str {
        "efficientip"
    }

    fn env_vars() -> &'static [&'static str] {
        &[
            "EfficientIP_Creds",
            "EfficientIP_Server",
            "EfficientIP_Token_Key",
            "EfficientIP_Token_Secret",
            "EfficientIP_DNS_Name",
            "EfficientIP_View",
        ]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let server = env
            .get("EfficientIP_Server")
            .ok_or_else(|| Error::Config("EfficientIP_Server required".into()))?
            .clone();

        let creds = env.get("EfficientIP_Creds").cloned();
        let token_key = env.get("EfficientIP_Token_Key").cloned();
        let token_secret = env.get("EfficientIP_Token_Secret").cloned();
        let dns_name = env.get("EfficientIP_DNS_Name").cloned();
        let dns_view = env.get("EfficientIP_View").cloned();

        if creds.is_none() && (token_key.is_none() || token_secret.is_none()) {
            return Err(Error::Config(
                "EfficientIP requires EfficientIP_Creds or both EfficientIP_Token_Key and EfficientIP_Token_Secret".into(),
            ));
        }

        Ok(Box::new(Efficientip {
            server,
            creds,
            token_key,
            token_secret,
            dns_name,
            dns_view,
        }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let mut url = format!(
            "https://{}/rest/dns_rr_add?rr_type=TXT&rr_ttl=300&rr_name={}&rr_value1={}",
            self.server,
            url_encode(name),
            url_encode(value),
        );
        if let Some(ref dn) = self.dns_name {
            if !dn.is_empty() {
                url.push_str(&format!("&dns_name={}", url_encode(dn)));
            }
        }
        if let Some(ref dv) = self.dns_view {
            if !dv.is_empty() {
                url.push_str(&format!("&dnsview_name={}", url_encode(dv)));
            }
        }

        let hdrs = self.build_headers("POST", &url);
        let hdr_refs: Vec<(&str, &str)> = hdrs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let resp = http::post(&url, b"", "application/octet-stream", &hdr_refs)
            .map_err(|e| Error::Provider(format!("efficientip add TXT: {e}")))?;
        if !resp.body.contains("ret_oid") {
            return Err(Error::Provider(format!(
                "efficientip add TXT: {} {}",
                resp.status, resp.body
            )));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let mut url = format!(
            "https://{}/rest/dns_rr_delete?rr_type=TXT&rr_name={}&rr_value1={}",
            self.server,
            url_encode(name),
            url_encode(value),
        );
        if let Some(ref dn) = self.dns_name {
            if !dn.is_empty() {
                url.push_str(&format!("&dns_name={}", url_encode(dn)));
            }
        }
        if let Some(ref dv) = self.dns_view {
            if !dv.is_empty() {
                url.push_str(&format!("&dnsview_name={}", url_encode(dv)));
            }
        }

        let hdrs = self.build_headers("DELETE", &url);
        let hdr_refs: Vec<(&str, &str)> = hdrs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let resp = http::delete(&url, &hdr_refs)
            .map_err(|e| Error::Provider(format!("efficientip rm TXT: {e}")))?;
        if !resp.body.contains("ret_oid") {
            return Err(Error::Provider(format!(
                "efficientip rm TXT: {} {}",
                resp.status, resp.body
            )));
        }
        Ok(())
    }
}

impl Efficientip {
    fn build_headers(&self, method: &str, url: &str) -> Vec<(String, String)> {
        let mut headers = Vec::new();
        headers.push(("Accept-Language".to_string(), "en-US".to_string()));

        if let (Some(tk), Some(ts)) = (&self.token_key, &self.token_secret) {
            if !tk.is_empty() && !ts.is_empty() {
                let epoch = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let ts_str = epoch.to_string();
                let sig_input = format!("{}\n{}\n{}\n{}", ts, ts_str, method, url);
                let sig = sha3_256_hex(sig_input.as_bytes());
                let auth = format!("SDS {}:{}", tk, sig);
                headers.push(("Authorization".to_string(), auth));
                headers.push(("X-SDS-TS".to_string(), ts_str));
                return headers;
            }
        }

        if let Some(ref creds) = self.creds {
            let encoded = base64::encode_std(creds.as_bytes());
            headers.push(("Authorization".to_string(), format!("Basic {}", encoded)));
        }

        headers
    }
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0xf) as usize] as char);
            }
        }
    }
    out
}

const HEX: &[u8; 16] = b"0123456789ABCDEF";

fn sha3_256_hex(input: &[u8]) -> String {
    let digest = sha3_256(input);
    base64::hex(&digest)
}

fn sha3_256(input: &[u8]) -> [u8; 32] {
    const RATE: usize = 136;
    let mut state = [0u64; 25];

    let chunks = input.len() / RATE;
    for i in 0..chunks {
        let block = &input[i * RATE..(i + 1) * RATE];
        absorb_block(&mut state, block);
    }

    let remaining = &input[chunks * RATE..];
    let mut last = vec![0u8; RATE];
    last[..remaining.len()].copy_from_slice(remaining);
    last[remaining.len()] = 0x06;
    last[RATE - 1] |= 0x80;
    absorb_block(&mut state, &last);

    let mut output = [0u8; 32];
    for i in 0..4 {
        let bytes = state[i].to_le_bytes();
        output[i * 8..(i + 1) * 8].copy_from_slice(&bytes);
    }
    output
}

fn absorb_block(state: &mut [u64; 25], block: &[u8]) {
    for i in 0..17 {
        let mut b = [0u8; 8];
        b.copy_from_slice(&block[i * 8..(i + 1) * 8]);
        state[i] ^= u64::from_le_bytes(b);
    }
    keccak_f1600(state);
}

fn keccak_f1600(state: &mut [u64; 25]) {
    const RC: [u64; 24] = [
        0x0000000000000001, 0x0000000000008082, 0x800000000000808A, 0x8000000080008000,
        0x000000000000808B, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
        0x000000000000008A, 0x0000000000000088, 0x0000000080008009, 0x000000008000000A,
        0x000000008000808B, 0x800000000000008B, 0x8000000000008089, 0x8000000000008003,
        0x8000000000008002, 0x8000000000000080, 0x000000000000800A, 0x800000008000000A,
        0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000000008008,
    ];
    const ROT: [u32; 25] = [
        0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39, 41, 45, 15, 21, 8, 18, 2, 61,
        56, 14,
    ];
    const PI: [usize; 25] = [
        0, 6, 12, 18, 24, 3, 9, 10, 16, 22, 1, 7, 13, 19, 20, 4, 5, 11, 17, 23, 2, 8, 14, 15,
        21,
    ];

    for round in 0..24 {
        let mut c = [0u64; 5];
        for x in 0..5 {
            c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
        }
        let mut d = [0u64; 5];
        for x in 0..5 {
            d[x] = c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1);
        }
        for x in 0..25 {
            state[x] ^= d[x % 5];
        }

        let mut b = [0u64; 25];
        for i in 0..25 {
            b[i] = state[PI[i]].rotate_left(ROT[PI[i]]);
        }

        for x in 0..5 {
            for y in 0..5 {
                state[x + 5 * y] = b[x + 5 * y] ^ (!b[(x + 1) % 5 + 5 * y] & b[(x + 2) % 5 + 5 * y]);
            }
        }

        state[0] ^= RC[round];
    }
}
