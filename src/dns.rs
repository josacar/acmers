use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

const TYPE_CNAME: u16 = 5;
const TYPE_TXT: u16 = 16;

pub fn resolvers(overrides: &[String]) -> Vec<String> {
    if !overrides.is_empty() {
        return overrides
            .iter()
            .flat_map(|s| s.split(','))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    let mut found = Vec::new();
    if let Ok(contents) = std::fs::read_to_string("/etc/resolv.conf") {
        for line in contents.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("nameserver") {
                let addr = rest.split_whitespace().next().unwrap_or("");
                if !addr.is_empty() {
                    found.push(addr.to_string());
                }
            }
        }
    }
    found
}

pub fn poll_txt(
    name: &str,
    expected: &str,
    servers: &[String],
    timeout: Duration,
) -> Result<(), String> {
    if servers.is_empty() {
        return Err("no DNS resolvers configured".into());
    }
    let deadline = Instant::now() + timeout;
    loop {
        for server in servers {
            match query_txt(server, name, Duration::from_secs(5)) {
                Ok(values) => {
                    if values.iter().any(|v| v == expected) {
                        return Ok(());
                    }
                }
                Err(e) => eprintln!("dns: query {server} failed: {e}"),
            }
        }
        let now = Instant::now();
        if now >= deadline {
            return Err(format!(
                "TXT record {name} with value {expected} not visible after {}s",
                timeout.as_secs()
            ));
        }
        let remaining = deadline - now;
        std::thread::sleep(remaining.min(Duration::from_secs(2)));
    }
}

pub fn query_txt(server: &str, name: &str, timeout: Duration) -> Result<Vec<String>, String> {
    query_txt_depth(server, name, timeout, 0)
}

fn query_txt_depth(
    server: &str,
    name: &str,
    timeout: Duration,
    depth: usize,
) -> Result<Vec<String>, String> {
    if depth > 5 {
        return Err("CNAME chain too deep".into());
    }
    let address = resolver_address(server)?;
    let id = query_id();
    let query = build_query(id, name)?;

    let bind: SocketAddr = if address.is_ipv4() {
        "0.0.0.0:0".parse().unwrap()
    } else {
        "[::]:0".parse().unwrap()
    };
    let socket = UdpSocket::bind(bind).map_err(|e| format!("bind UDP socket: {e}"))?;
    socket
        .set_read_timeout(Some(timeout))
        .map_err(|e| format!("set timeout: {e}"))?;
    socket
        .send_to(&query, address)
        .map_err(|e| format!("send DNS query: {e}"))?;

    let mut buf = [0u8; 4096];
    let (len, _) = socket
        .recv_from(&mut buf)
        .map_err(|e| format!("receive DNS response: {e}"))?;
    let (txt, cname) = parse_response(&buf[..len], id)?;

    if txt.is_empty()
        && let Some(target) = cname
    {
        return query_txt_depth(server, &target, timeout, depth + 1);
    }
    Ok(txt)
}

fn parse_response(response: &[u8], id: u16) -> Result<(Vec<String>, Option<String>), String> {
    if response.len() < 12 {
        return Err("short DNS response".into());
    }
    let response_id = u16::from_be_bytes([response[0], response[1]]);
    if response_id != id {
        return Err("DNS response ID mismatch".into());
    }
    let flags = u16::from_be_bytes([response[2], response[3]]);
    let rcode = flags & 0x000f;
    if rcode != 0 {
        return Err(format!("DNS rcode {rcode}"));
    }

    let qdcount = u16::from_be_bytes([response[4], response[5]]) as usize;
    let ancount = u16::from_be_bytes([response[6], response[7]]) as usize;

    let mut pos = 12usize;
    for _ in 0..qdcount {
        let (_, next) = read_name(response, pos)?;
        pos = next + 4;
    }

    let mut txt = Vec::new();
    let mut cname = None;
    for _ in 0..ancount {
        let (_, next) = read_name(response, pos)?;
        pos = next;
        if pos + 10 > response.len() {
            return Err("truncated DNS answer".into());
        }
        let typ = u16::from_be_bytes([response[pos], response[pos + 1]]);
        let rdlength = u16::from_be_bytes([response[pos + 8], response[pos + 9]]) as usize;
        pos += 10;
        if pos + rdlength > response.len() {
            return Err("truncated DNS rdata".into());
        }
        let rdata = &response[pos..pos + rdlength];
        match typ {
            TYPE_TXT => parse_txt(rdata, &mut txt),
            TYPE_CNAME => {
                if let Ok((target, _)) = read_name(response, pos) {
                    cname = Some(target);
                }
            }
            _ => {}
        }
        pos += rdlength;
    }

    Ok((txt, cname))
}

fn resolver_address(server: &str) -> Result<SocketAddr, String> {
    if let Ok(addr) = server.parse::<SocketAddr>() {
        return Ok(addr);
    }
    if let Ok(ip) = server.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, 53));
    }
    let with_port = format!("{server}:53");
    with_port
        .parse::<SocketAddr>()
        .map_err(|e| format!("invalid resolver '{server}': {e}"))
}

fn query_id() -> u16 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    (nanos as u16) ^ (std::process::id() as u16)
}

fn build_query(id: u16, name: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(&id.to_be_bytes());
    out.extend_from_slice(&0x0100u16.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());

    for label in name.trim_end_matches('.').split('.') {
        if label.is_empty() {
            continue;
        }
        if label.len() > 63 {
            return Err(format!("DNS label too long: {label}"));
        }
        out.push(label.len() as u8);
        out.extend_from_slice(label.as_bytes());
    }
    out.push(0);
    out.extend_from_slice(&TYPE_TXT.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    Ok(out)
}

fn read_name(data: &[u8], mut pos: usize) -> Result<(String, usize), String> {
    let mut labels = Vec::new();
    let mut next = pos;
    let mut jumped = false;
    let mut hops = 0usize;
    loop {
        if pos >= data.len() {
            return Err("DNS name out of bounds".into());
        }
        let len = data[pos];
        if len & 0xc0 == 0xc0 {
            if pos + 1 >= data.len() {
                return Err("truncated DNS pointer".into());
            }
            let ptr = (((len & 0x3f) as usize) << 8) | data[pos + 1] as usize;
            if !jumped {
                next = pos + 2;
                jumped = true;
            }
            pos = ptr;
            hops += 1;
            if hops > 32 {
                return Err("DNS compression loop".into());
            }
        } else if len == 0 {
            pos += 1;
            if !jumped {
                next = pos;
            }
            break;
        } else {
            let label_len = len as usize;
            if pos + 1 + label_len > data.len() {
                return Err("truncated DNS label".into());
            }
            labels.push(String::from_utf8_lossy(&data[pos + 1..pos + 1 + label_len]).to_string());
            pos += 1 + label_len;
        }
    }
    Ok((labels.join("."), next))
}

fn parse_txt(rdata: &[u8], out: &mut Vec<String>) {
    let mut pos = 0usize;
    while pos < rdata.len() {
        let len = rdata[pos] as usize;
        pos += 1;
        if pos + len > rdata.len() {
            break;
        }
        out.push(String::from_utf8_lossy(&rdata[pos..pos + len]).to_string());
        pos += len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_name(name: &str) -> Vec<u8> {
        let mut out = Vec::new();
        for label in name.split('.') {
            out.push(label.len() as u8);
            out.extend_from_slice(label.as_bytes());
        }
        out.push(0);
        out
    }

    fn response(id: u16, name: &str, records: &[(u16, Vec<u8>)]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&id.to_be_bytes());
        out.extend_from_slice(&0x8180u16.to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&(records.len() as u16).to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes());
        out.extend_from_slice(&encode_name(name));
        out.extend_from_slice(&TYPE_TXT.to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes());
        for (typ, rdata) in records {
            out.extend_from_slice(&[0xc0, 0x0c]);
            out.extend_from_slice(&typ.to_be_bytes());
            out.extend_from_slice(&1u16.to_be_bytes());
            out.extend_from_slice(&60u32.to_be_bytes());
            out.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
            out.extend_from_slice(rdata);
        }
        out
    }

    fn txt(value: &str) -> Vec<u8> {
        let mut out = vec![value.len() as u8];
        out.extend_from_slice(value.as_bytes());
        out
    }

    #[test]
    fn parses_txt_answer() {
        let data = response(
            7,
            "_acme-challenge.example.com",
            &[(TYPE_TXT, txt("abc123"))],
        );
        let (values, cname) = parse_response(&data, 7).unwrap();
        assert_eq!(values, vec!["abc123"]);
        assert!(cname.is_none());
    }

    #[test]
    fn parses_multiple_txt_values() {
        let data = response(
            9,
            "_acme-challenge.example.com",
            &[(TYPE_TXT, txt("first")), (TYPE_TXT, txt("second"))],
        );
        let (values, _) = parse_response(&data, 9).unwrap();
        assert_eq!(values, vec!["first", "second"]);
    }

    #[test]
    fn parses_cname_answer() {
        let data = response(
            11,
            "_acme-challenge.example.com",
            &[(TYPE_CNAME, encode_name("target.example.net"))],
        );
        let (values, cname) = parse_response(&data, 11).unwrap();
        assert!(values.is_empty());
        assert_eq!(cname.as_deref(), Some("target.example.net"));
    }

    #[test]
    fn rejects_mismatched_id() {
        let data = response(3, "example.com", &[(TYPE_TXT, txt("x"))]);
        assert!(parse_response(&data, 4).is_err());
    }

    #[test]
    fn follows_compressed_pointer() {
        let mut data = vec![0u8; 12];
        data.extend_from_slice(&encode_name("www.example.com"));
        data.extend_from_slice(&[0xc0, 0x0c]);
        let offset = 12;
        let (name, next) = read_name(&data, offset + 17).unwrap();
        assert_eq!(name, "www.example.com");
        assert_eq!(next, offset + 19);
    }

    #[test]
    fn builds_query() {
        let query = build_query(42, "_acme-challenge.example.com").unwrap();
        assert_eq!(&query[0..2], &42u16.to_be_bytes());
        assert_eq!(&query[4..6], &1u16.to_be_bytes());
        assert!(query.len() > 12);
        assert_eq!(query[query.len() - 4..], [0, TYPE_TXT as u8, 0, 1]);
    }
}
