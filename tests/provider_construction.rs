use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;

use acmers::providers;

struct FastMock {
    port: u16,
    handle: Option<thread::JoinHandle<()>>,
    stop: Option<std::sync::mpsc::Sender<()>>,
}

impl FastMock {
    fn new(status: u16, body: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = std::sync::mpsc::channel();
        let h = thread::spawn(move || {
            listener.set_nonblocking(true).unwrap();
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buf = [0u8; 4096];
                        let _ = stream.read(&mut buf);
                        let resp = format!(
                            "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(resp.as_bytes());
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => break,
                }
                if rx.try_recv().is_ok() { break; }
                thread::sleep(std::time::Duration::from_millis(1));
            }
        });
        FastMock { port, handle: Some(h), stop: Some(tx) }
    }
    fn url(&self) -> String { format!("http://127.0.0.1:{}", self.port) }
}

impl Drop for FastMock {
    fn drop(&mut self) {
        if let Some(tx) = self.stop.take() { let _ = tx.send(()); }
        if let Some(h) = self.handle.take() { let _ = h.join(); }
    }
}

#[test]
fn test_all_providers_have_unique_slugs() {
    let mut slugs = std::collections::HashSet::new();
    for p in providers::list() {
        assert!(slugs.insert(p.slug), "duplicate slug: {}", p.slug);
    }
}

#[test]
fn test_all_providers_have_env_vars() {
    for meta in providers::list() {
        assert!(!meta.env_vars.is_empty(), "provider '{}' has no env vars defined", meta.slug);
    }
}

#[test]
fn test_all_providers_have_names() {
    for meta in providers::list() {
        assert!(!meta.name.is_empty(), "provider '{}' has empty name", meta.slug);
        assert!(!meta.slug.is_empty(), "provider has empty slug");
    }
}

#[test]
fn test_provider_count() {
    let count = providers::list().len();
    assert!(count >= 150, "expected at least 150 providers, got {count}");
}

#[test]
fn test_all_providers_construct_without_panicking() {
    let mock = FastMock::new(200, "{}");
    acmers::http::set_test_base(&mock.url());

    let mut constructed = 0;
    let mut errors = 0;
    let total = providers::list().len();

    for (i, meta) in providers::list().iter().enumerate() {
        let mut env = HashMap::new();
        for var in meta.env_vars {
            env.insert(var.to_string(), "test-value-12345".to_string());
        }
        match (meta.create)(&env) {
            Ok(_) => constructed += 1,
            Err(_) => errors += 1,
        }
        if (i + 1) % 50 == 0 {
            println!("  progress: {}/{total} ({} ok, {} err)", i + 1, constructed, errors);
        }
    }
    println!("constructed: {constructed}/{total}, errors: {errors}");
    assert!(constructed + errors > 0, "no providers tested");
}
