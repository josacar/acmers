use serde_json::Value;

use crate::crypto::{AccountKey, KidOrJwk, sign_jws};
use crate::error::Error;
use crate::http;
use crate::json as j;

pub struct Authorization {
    pub identifier: Identifier,
    pub status: String,
    pub expires: Option<String>,
    pub challenges: Vec<Challenge>,
    pub wildcard: bool,
}

pub struct Identifier {
    pub typ: String,
    pub value: String,
}

pub struct Challenge {
    pub typ: String,
    pub url: String,
    pub token: Option<String>,
    pub status: String,
}

impl Challenge {
    pub fn key_authorization(&self, jwk_thumbprint: &str) -> String {
        let token = self.token.as_deref().unwrap_or("");
        format!("{}.{}", token, jwk_thumbprint)
    }
}

pub fn get_authorizations(
    account_url: &str,
    auth_urls: &[String],
    key: &AccountKey,
    get_nonce: &mut dyn FnMut() -> Result<String, Error>,
) -> Result<Vec<Authorization>, Error> {
    let mut auths = Vec::new();
    for url in auth_urls {
        let nonce = get_nonce()?;
        let jws = sign_jws(b"", &key.key_pair, &KidOrJwk::Kid(account_url.to_string()), &nonce, url)?;
        let resp = http::post(
            url,
            &serde_json::to_vec(&jws).unwrap(),
            "application/jose+json",
            &[],
        )
        .map_err(|e| Error::Acme { status: 0, detail: e, error_type: "http".into() })?;

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("parse authz: {e}")))?;

        let identifier_val = j::get_value_required(&v, &["identifier"])?;
        let challenges_arr = j::get_array_required(&v, &["challenges"])?;

        let mut challenges = Vec::new();
        for ch in challenges_arr {
            challenges.push(Challenge {
                typ: j::get_string_required(ch, &["type"])?.to_string(),
                url: j::get_string_required(ch, &["url"])?.to_string(),
                token: j::get_string(ch, &["token"]).map(|s| s.to_string()),
                status: j::get_string_required(ch, &["status"])?.to_string(),
            });
        }

        auths.push(Authorization {
            identifier: Identifier {
                typ: j::get_string_required(identifier_val, &["type"])?.to_string(),
                value: j::get_string_required(identifier_val, &["value"])?.to_string(),
            },
            status: j::get_string_required(&v, &["status"])?.to_string(),
            expires: j::get_string(&v, &["expires"]).map(|s| s.to_string()),
            challenges,
            wildcard: v.get("wildcard").and_then(|w| w.as_bool()).unwrap_or(false),
        });
    }
    Ok(auths)
}

pub fn respond_to_challenge(
    challenge_url: &str,
    account_url: &str,
    key: &AccountKey,
    get_nonce: &mut dyn FnMut() -> Result<String, Error>,
) -> Result<(), Error> {
    let nonce = get_nonce()?;
    let payload = serde_json::json!({});
    let jws = sign_jws(
        &serde_json::to_vec(&payload).unwrap(),
        &key.key_pair,
        &KidOrJwk::Kid(account_url.to_string()),
        &nonce,
        challenge_url,
    )?;

    let resp = http::post(
        challenge_url,
        &serde_json::to_vec(&jws).unwrap(),
        "application/jose+json",
        &[],
    )
    .map_err(|e| Error::Acme { status: 0, detail: e, error_type: "http".into() })?;

    let v: Value = serde_json::from_str(&resp.body)
        .map_err(|e| Error::Json(format!("parse challenge response: {e}")))?;
    let status = j::get_string_required(&v, &["status"])?;
    if status == "invalid" {
        let err = j::get_string(&v, &["error", "detail"]).unwrap_or("unknown");
        return Err(Error::Acme {
            status: resp.status,
            detail: err.to_string(),
            error_type: "challenge_failed".into(),
        });
    }
    Ok(())
}

pub fn poll_challenge(
    challenge_url: &str,
    account_url: &str,
    key: &AccountKey,
    get_nonce: &mut dyn FnMut() -> Result<String, Error>,
) -> Result<String, Error> {
    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let nonce = get_nonce()?;
        let jws = crate::crypto::sign_jws(
            b"",
            &key.key_pair,
            &KidOrJwk::Kid(account_url.to_string()),
            &nonce,
            challenge_url,
        )?;
        let resp = http::post(
            challenge_url,
            &serde_json::to_vec(&jws).unwrap(),
            "application/jose+json",
            &[],
        )
        .map_err(|e| Error::Acme { status: 0, detail: e, error_type: "http".into() })?;

        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("parse challenge poll: {e}")))?;
        let status = j::get_string_required(&v, &["status"])?;
        match status {
            "valid" => return Ok(status.to_string()),
            "invalid" => {
                let err = j::get_string(&v, &["error", "detail"]).unwrap_or("challenge invalid");
                return Err(Error::Acme {
                    status: resp.status,
                    detail: err.to_string(),
                    error_type: "challenge_failed".into(),
                });
            }
            _ => continue,
        }
    }
    Err(Error::Acme {
        status: 0,
        detail: "timed out waiting for challenge".into(),
        error_type: "timeout".into(),
    })
}

pub fn start_http_server(port: u16, challenges: std::collections::HashMap<String, String>) -> std::thread::JoinHandle<()> {
    let listener = std::net::TcpListener::bind(("0.0.0.0", port)).unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            if let Ok(mut stream) = stream {
                let mut buf = [0u8; 4096];
                if let Ok(n) = std::io::Read::read(&mut stream, &mut buf) {
                    let req = std::str::from_utf8(&buf[..n]).unwrap_or("");
                    if let Some(line) = req.lines().next() {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            let path = parts[1];
                            let token = path.trim_start_matches("/.well-known/acme-challenge/");
                            if let Some(content) = challenges.get(token) {
                                let resp = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{}", content.len(), content);
                                let _ = std::io::Write::write_all(&mut stream, resp.as_bytes());
                            }
                        }
                    }
                }
            }
        }
    })
}

pub fn start_http_challenges(
    challenges: &[(String, String)],
) -> std::thread::JoinHandle<()> {
    use std::collections::HashMap;
    let map: HashMap<String, String> = challenges.iter().cloned().collect();
    let listener = match std::net::TcpListener::bind("0.0.0.0:80") {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Cannot bind port 80: {e}. Run with sudo or use DNS-01 challenge.");
            std::process::exit(1);
        }
    };
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            if let Ok(mut stream) = stream {
                let mut buf = [0u8; 4096];
                if let Ok(n) = std::io::Read::read(&mut stream, &mut buf) {
                    let req = std::str::from_utf8(&buf[..n]).unwrap_or("");
                    if let Some(line) = req.lines().next() {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            let path = parts[1];
                            let token = path.trim_start_matches("/.well-known/acme-challenge/");
                            if let Some(content) = map.get(token) {
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{}",
                                    content.len(), content
                                );
                                let _ = std::io::Write::write_all(&mut stream, resp.as_bytes());
                            }
                        }
                    }
                }
            }
        }
    })
}
