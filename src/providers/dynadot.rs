use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Dynadot {
    key: String,
}

impl DnsProvider for Dynadot {
    fn slug() -> &'static str {
        "dynadot"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DYNADOT_Key"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let key = env.get("DYNADOT_Key")
            .ok_or_else(|| Error::Config("DYNADOT_Key required".into()))?
            .clone();
        Ok(Box::new(Dynadot { key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = self.resolve_domain(domain)?;
        let subdomain = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let url = format!(
            "https://api.dynadot.com/api3.xml?key={}&command=add_domain_record&domain_id={domain_id}&record_type=TXT&subdomain={}&value={}&ttl=120",
            self.key,
            url_encode(subdomain),
            url_encode(value),
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("dynadot add TXT: {e}")))?;
        if resp.status >= 400 || resp.body.contains("<ErrorCode>") {
            return Err(Error::Provider(format!("dynadot add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let domain_id = match self.resolve_domain(domain) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let record_id = match self.find_record(&domain_id, domain, name, value) {
            Ok(Some(id)) => id,
            _ => return Ok(()),
        };
        let url = format!(
            "https://api.dynadot.com/api3.xml?key={}&command=remove_domain_record&domain_id={domain_id}&record_id={record_id}",
            self.key,
        );
        http::get(&url, &[]).ok();
        Ok(())
    }
}

impl Dynadot {
    fn resolve_domain(&self, domain: &str) -> Result<String, Error> {
        let url = format!("https://api.dynadot.com/api3.xml?key={}&command=list_domain", self.key);
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("dynadot list domains: {e}")))?;

        let body = &resp.body;
        let domains = parse_xml_tags(body, "ListDomainsContent", &["DomainName", "DomainId"]);
        for d in domains {
            if let (Some(nm), Some(id)) = (d.get("DomainName"), d.get("DomainId")) {
                if domain == nm || domain.ends_with(&format!(".{nm}")) {
                    return Ok(id.clone());
                }
            }
        }
        Err(Error::Provider(format!("dynadot: domain {domain} not found")))
    }

    fn find_record(&self, domain_id: &str, domain: &str, name: &str, value: &str) -> Result<Option<String>, Error> {
        let subdomain = name.strip_suffix(&format!(".{domain}")).unwrap_or(name);
        let url = format!(
            "https://api.dynadot.com/api3.xml?key={}&command=list_domain_record&domain_id={domain_id}",
            self.key,
        );
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("dynadot list records: {e}")))?;

        let body = &resp.body;
        let records = parse_xml_tags(body, "ListDomainRecordsContent", &["RecordId", "Subdomain", "RecordType", "Value"]);
        for r in records {
            if r.get("RecordType").map(|s| s.as_str()) == Some("TXT")
                && r.get("Subdomain").map(|s| s.as_str()) == Some(subdomain)
                && r.get("Value").map(|s| s.as_str()) == Some(value)
            {
                if let Some(id) = r.get("RecordId") {
                    return Ok(Some(id.clone()));
                }
            }
        }
        Ok(None)
    }
}

fn parse_xml_tags(xml: &str, container: &str, fields: &[&str]) -> Vec<HashMap<String, String>> {
    let mut results = Vec::new();
    let open = format!("<{container}>");
    let close = format!("</{container}>");

    let mut pos = 0;
    while let Some(start) = xml[pos..].find(&open) {
        let block_start = pos + start + open.len();
        let block_end = match xml[block_start..].find(&close) {
            Some(end) => block_start + end,
            None => break,
        };
        let block = &xml[block_start..block_end];

        let mut map = HashMap::new();
        for &field in fields {
            let tag_open = format!("<{field}>");
            let tag_close = format!("</{field}>");
            if let Some(f_start) = block.find(&tag_open) {
                let val_start = f_start + tag_open.len();
                if let Some(f_end) = block[val_start..].find(&tag_close) {
                    let val = block[val_start..val_start + f_end].to_string();
                    map.insert(field.to_string(), val);
                }
            }
        }
        results.push(map);
        pos = block_end + close.len();
    }
    results
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => {
                out.push('%');
                out.push(to_hex((b >> 4) & 0xf));
                out.push(to_hex(b & 0xf));
            }
        }
    }
    out
}

fn to_hex(n: u8) -> char {
    if n < 10 { (b'0' + n) as char } else { (b'A' + n - 10) as char }
}
