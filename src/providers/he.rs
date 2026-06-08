use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct He {
    username: String,
    password: String,
}

impl DnsProvider for He {
    fn slug() -> &'static str {
        "he"
    }

    fn env_vars() -> &'static [&'static str] {
        &["HE_Username", "HE_Password"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(He {
            username: env.get("HE_Username")
                .ok_or_else(|| Error::Config("HE_Username required".into()))?
                .clone(),
            password: env.get("HE_Password")
                .ok_or_else(|| Error::Config("HE_Password required".into()))?
                .clone(),
        }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = self.find_zone(name)?;

        let form = format!(
            "email={}&pass={}&account=&menu=edit_zone&Type=TXT&hosted_dns_zoneid={}&hosted_dns_recordid=&hosted_dns_editzone=1&Priority=&Name={}&Content={}&TTL=300&hosted_dns_editrecord=Submit",
            url_encode_f(&self.username),
            url_encode_f(&self.password),
            url_encode_f(&zone_id),
            url_encode_f(name),
            url_encode_f(value),
        );
        let resp = http::post(
            "https://dns.he.net/",
            form.as_bytes(),
            "application/x-www-form-urlencoded",
            &[],
        )
        .map_err(|e| Error::Provider(format!("HE add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("HE add TXT: HTTP {}", resp.status)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone_id = match self.find_zone(name) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };

        let form = format!(
            "email={}&pass={}&hosted_dns_zoneid={}&menu=edit_zone&hosted_dns_editzone=",
            url_encode_f(&self.username),
            url_encode_f(&self.password),
            url_encode_f(&zone_id),
        );
        let resp = match http::post(
            "https://dns.he.net/",
            form.as_bytes(),
            "application/x-www-form-urlencoded",
            &[],
        ) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };

        let record_id = match find_record_id(&resp.body, name, value) {
            Some(id) => id,
            None => return Ok(()),
        };

        let del_form = format!(
            "email={}&pass={}&menu=edit_zone&hosted_dns_zoneid={}&hosted_dns_recordid={}&hosted_dns_editzone=1&hosted_dns_delrecord=1&hosted_dns_delconfirm=delete",
            url_encode_f(&self.username),
            url_encode_f(&self.password),
            url_encode_f(&zone_id),
            url_encode_f(&record_id),
        );
        let _ = http::post(
            "https://dns.he.net/",
            del_form.as_bytes(),
            "application/x-www-form-urlencoded",
            &[],
        );
        Ok(())
    }
}

impl He {
    fn find_zone(&self, domain: &str) -> Result<String, Error> {
        let body = format!(
            "email={}&pass={}",
            url_encode_f(&self.username),
            url_encode_f(&self.password),
        );
        let resp = http::post(
            "https://dns.he.net/",
            body.as_bytes(),
            "application/x-www-form-urlencoded",
            &[],
        )
        .map_err(|e| Error::Provider(format!("HE login: {e}")))?;

        if resp.body.contains(">Incorrect<") {
            return Err(Error::Provider("HE: login failed".into()));
        }

        let zones = parse_zones(&resp.body);
        if zones.is_empty() {
            return Err(Error::Provider("HE: could not parse zones".into()));
        }

        let parts: Vec<&str> = domain.split('.').collect();
        for i in 0..parts.len() {
            let attempt = parts[i..].join(".");
            for (zone_name, zone_id) in &zones {
                if *zone_name == attempt {
                    return Ok(zone_id.clone());
                }
            }
        }

        Err(Error::Provider(format!("HE: zone not found for {domain}")))
    }
}

fn parse_zones(html: &str) -> Vec<(String, String)> {
    let mut zones = Vec::new();

    let table_start = match html.find("id=\"domains_table\"") {
        Some(pos) => pos,
        None => return zones,
    };

    let table_section = &html[table_start..];
    let table_end = table_section.find("</table>").unwrap_or(table_section.len());
    let table_html = &table_section[..table_end];

    let rows: Vec<&str> = table_html.split("<tr").collect();

    for row in rows {
        if !row.contains("alt=\"edit\"") {
            continue;
        }

        let no_spaces: String = row.chars().filter(|c| !c.is_whitespace()).collect();
        let cells: Vec<&str> = no_spaces.split("<td").collect();
        for cell in cells {
            if !cell.contains("hosted_dns_zoneid") {
                continue;
            }

            let zone_id = match extract_zone_id(cell) {
                Some(id) => id,
                None => continue,
            };
            let zone_name = match extract_zone_name(cell) {
                Some(name) => name,
                None => continue,
            };
            zones.push((zone_name, zone_id));
        }
    }

    zones
}

fn extract_zone_id(cell: &str) -> Option<String> {
    let marker = "hosted_dns_zoneid=";
    let pos = cell.find(marker)? + marker.len();
    let rest = &cell[pos..];
    let end = rest.find(|c: char| !c.is_ascii_digit())?;
    let id = &rest[..end];
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

fn extract_zone_name(cell: &str) -> Option<String> {
    let marker = "name=\"";
    let pos = cell.find(marker)? + marker.len();
    let rest = &cell[pos..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn find_record_id(html: &str, full_domain: &str, txt_value: &str) -> Option<String> {
    let rows: Vec<&str> = html.split("<tr").collect();
    for row in rows {
        if !row.contains(full_domain) {
            continue;
        }
        if !row.contains("\"dns_tr\"") {
            continue;
        }
        if !row.contains(txt_value) {
            continue;
        }
        let fields: Vec<&str> = row.split('"').collect();
        if fields.len() >= 4 {
            let id = fields[3].trim();
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}

fn url_encode_f(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push(hex_f((b >> 4) & 0xf));
                out.push(hex_f(b & 0xf));
            }
        }
    }
    out
}

fn hex_f(n: u8) -> char {
    if n < 10 {
        (b'0' + n) as char
    } else {
        (b'A' + n - 10) as char
    }
}
