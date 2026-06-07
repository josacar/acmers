use std::collections::HashMap;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Pleskxml {
    host: String,
    basic_auth: String,
}

impl DnsProvider for Pleskxml {
    fn slug() -> &'static str {
        "pleskxml"
    }

    fn env_vars() -> &'static [&'static str] {
        &["PLESKXML_Username", "PLESKXML_Password", "PLESKXML_Host"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env.get("PLESKXML_Username")
            .ok_or_else(|| Error::Config("PLESKXML_Username required".into()))?
            .clone();
        let password = env.get("PLESKXML_Password")
            .ok_or_else(|| Error::Config("PLESKXML_Password required".into()))?
            .clone();
        let host = env.get("PLESKXML_Host")
            .ok_or_else(|| Error::Config("PLESKXML_Host required".into()))?
            .clone();
        let creds = format!("{username}:{password}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Pleskxml { host, basic_auth }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let site_id = self.resolve_site(domain)?;
        let record_name = extract_record_name(name, domain);
        let record_id = self.create_record(&site_id, &record_name, value)?;
        let _ = record_id;
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let site_id = match self.resolve_site(domain) {
            Ok(id) => id,
            Err(_) => return Ok(()),
        };
        let record_name = extract_record_name(name, domain);
        let record_id = match self.find_record_id(&site_id, &record_name, value) {
            Ok(Some(id)) => id,
            Ok(None) => return Ok(()),
            Err(_) => return Ok(()),
        };
        let _ = self.delete_record(&record_id);
        Ok(())
    }
}

fn extract_record_name(name: &str, domain: &str) -> String {
    if name == domain || name.is_empty() {
        String::new()
    } else if name.ends_with(domain) {
        let prefix_end = name.len() - domain.len();
        if prefix_end > 0 {
            name[..prefix_end - 1].to_string()
        } else {
            String::new()
        }
    } else {
        name.to_string()
    }
}

impl Pleskxml {
    fn api_url(&self) -> String {
        format!("https://{}:8443/enterprise/control/agent.php", self.host)
    }

    fn plesk_request(&self, xml_body: &str) -> Result<http::Response, Error> {
        let url = self.api_url();
        http::post(&url, xml_body.as_bytes(), "text/xml", &[
            ("Authorization", &self.basic_auth),
        ])
        .map_err(|e| Error::Provider(format!("pleskxml request: {e}")))
    }

    fn resolve_site(&self, domain: &str) -> Result<String, Error> {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<packet version="1.6.9.0">
  <site>
    <get>
      <filter>
        <name>{d}</name>
      </filter>
      <dataset>
        <hosting/>
      </dataset>
    </get>
  </site>
</packet>"#,
            d = plesk_xml_escape(domain)
        );
        let resp = self.plesk_request(&xml)?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("pleskxml resolve site: HTTP {} {}", resp.status, resp.body)));
        }
        plesk_find_int(&resp.body, "id")
            .ok_or_else(|| Error::Provider(format!("pleskxml: site not found for {domain}")))
    }

    fn find_record_id(&self, site_id: &str, record_name: &str, value: &str) -> Result<Option<String>, Error> {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<packet version="1.6.9.0">
  <dns>
    <get_records>
      <filter>
        <site-id>{sid}</site-id>
      </filter>
    </get_records>
  </dns>
</packet>"#,
            sid = site_id
        );
        let resp = match self.plesk_request(&xml) {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };
        if resp.status >= 400 {
            return Ok(None);
        }
        let body = &resp.body;
        let mut pos = 0;
        while let Some(start) = body[pos..].find("<record>") {
            let rec_start = pos + start;
            let rec_end = match body[rec_start..].find("</record>") {
                Some(e) => rec_start + e + 9,
                None => break,
            };
            let record = &body[rec_start..rec_end];
            let r_type = plesk_find_text(record, "type");
            let r_name = plesk_find_text(record, "host");
            let r_value = plesk_find_text(record, "value");
            if r_type.as_deref() == Some("TXT")
                && r_name.as_deref() == Some(record_name)
                && r_value.as_deref() == Some(value)
            {
                if let Some(id) = plesk_find_int(record, "id") {
                    return Ok(Some(id));
                }
            }
            pos = rec_end;
        }
        Ok(None)
    }

    fn create_record(&self, site_id: &str, record_name: &str, value: &str) -> Result<String, Error> {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<packet version="1.6.9.0">
  <dns>
    <add_records>
      <site-id>{sid}</site-id>
      <record>
        <name>{rn}</name>
        <type>TXT</type>
        <opt/>
        <value>{v}</value>
      </record>
    </add_records>
  </dns>
</packet>"#,
            sid = site_id,
            rn = plesk_xml_escape(record_name),
            v = plesk_xml_escape(value)
        );
        let resp = self.plesk_request(&xml)?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("pleskxml add record: HTTP {} {}", resp.status, resp.body)));
        }
        plesk_find_int(&resp.body, "id")
            .ok_or_else(|| Error::Provider("pleskxml: could not extract record id from create response".into()))
    }

    fn delete_record(&self, record_id: &str) -> Result<(), Error> {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<packet version="1.6.9.0">
  <dns>
    <del_records>
      <filter>
        <id>{rid}</id>
      </filter>
    </del_records>
  </dns>
</packet>"#,
            rid = record_id
        );
        let resp = self.plesk_request(&xml)?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("pleskxml delete record: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }
}

fn plesk_xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn plesk_find_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    if let Some(start) = xml.find(&open) {
        let content_start = start + open.len();
        if let Some(end) = xml[content_start..].find(&close) {
            return Some(xml[content_start..content_start + end].to_string());
        }
    }
    None
}

fn plesk_find_int(xml: &str, tag: &str) -> Option<String> {
    plesk_find_text(xml, tag)
}
