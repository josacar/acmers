use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::panic::catch_unwind;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

type Handler = Arc<dyn Fn(&str, &str, &[u8], &HashMap<String, String>) -> (u16, String, HashMap<String, String>) + Send + Sync>;

pub struct MockServer {
    port: u16,
    handle: Option<JoinHandle<()>>,
    stop_tx: Option<std::sync::mpsc::Sender<()>>,
}

impl MockServer {
    pub fn new(handler: Handler) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();

        let handle = thread::spawn(move || {
            listener.set_nonblocking(true).unwrap();
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buf = [0u8; 16384];
                        if let Ok(n) = stream.read(&mut buf) {
                            let req = String::from_utf8_lossy(&buf[..n]).to_string();
                            let (method, path, body, headers) = parse_request(&req);
                            let result = catch_unwind(std::panic::AssertUnwindSafe(|| {
                                handler(&method, &path, &body, &headers)
                            }));
                            let (status, resp_body, resp_headers) = match result {
                                Ok(r) => r,
                                Err(_) => (500, r#"{"error":"mock handler panic"}"#.to_string(), HashMap::new()),
                            };
                            let mut response = format!("HTTP/1.1 {status} OK\r\n");
                            for (k, v) in &resp_headers {
                                response.push_str(&format!("{k}: {v}\r\n"));
                            }
                            if resp_headers.get("Content-Type").is_none() {
                                response.push_str("Content-Type: application/json\r\n");
                            }
                            response.push_str(&format!("Content-Length: {}\r\n", resp_body.len()));
                            response.push_str("\r\n");
                            response.push_str(&resp_body);
                            let _ = stream.write_all(response.as_bytes());
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => break,
                }
                if stop_rx.try_recv().is_ok() {
                    break;
                }
                thread::sleep(std::time::Duration::from_millis(1));
            }
        });

        MockServer {
            port,
            handle: Some(handle),
            stop_tx: Some(stop_tx),
        }
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn url_for(&self, original_host: &str) -> String {
        format!("http://127.0.0.1:{}/{}", self.port, original_host)
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn parse_request(req: &str) -> (String, String, Vec<u8>, HashMap<String, String>) {
    let parts: Vec<&str> = req.splitn(2, "\r\n\r\n").collect();
    let header_part = parts[0];
    let body_bytes = if parts.len() > 1 { parts[1].as_bytes().to_vec() } else { vec![] };

    let lines: Vec<&str> = header_part.split("\r\n").collect();
    if lines.is_empty() {
        return (String::new(), String::new(), body_bytes, HashMap::new());
    }

    let req_parts: Vec<&str> = lines[0].split_whitespace().collect();
    let method = req_parts.first().map(|s| s.to_string()).unwrap_or_default();
    let path = req_parts.get(1).map(|s| s.to_string()).unwrap_or_default();

    let mut headers = HashMap::new();
    for line in &lines[1..] {
        if let Some((k, v)) = line.split_once(": ") {
            headers.insert(k.to_lowercase(), v.to_string());
        }
    }

    (method, path, body_bytes, headers)
}

