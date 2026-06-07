use std::collections::HashMap;
use serde_json::Value;

use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Durabledns {
    user: String,
    key: String,
}

impl DnsProvider for Durabledns {
    fn slug() -> &'static str {
        "durabledns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["DURABLEDNS_User", "DURABLEDNS_Key"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let user = env.get("DURABLEDNS_User")
            .ok_or_else(|| Error::Config("DURABLEDNS_User required".into()))?
            .clone();
        let key = env.get("DURABLEDNS_Key")
            .ok_or_else(|| Error::Config("DURABLEDNS_Key required".into()))?
            .clone();
        Ok(Box::new(Durabledns { user, key }))
    }

    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let base = "https://api.durabledns.com";
        let _zone_id = self.zone_id(domain)?;
        let url = format!("{base}/dns/create_record.php?apiuser={}&apikey={}&zonename={domain}&name={name}&type=TXT&content={value}&ttl=120",
            self.user, self.key);
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("durabledns create: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("durabledns create: {} {}", resp.status, resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        let base = "https://api.durabledns.com";
        let record_id = match self.find_record(domain, name, value) {
            Some(id) => id,
            None => return Ok(()),
        };
        let url = format!("{base}/dns/delete_record.php?apiuser={}&apikey={}&zonename={domain}&recordid={record_id}",
            self.user, self.key);
        http::get(&url, &[]).ok();
        Ok(())
    }
}

impl Durabledns {
    fn zone_id(&self, domain: &str) -> Result<String, Error> {
        let url = format!("https://api.durabledns.com/dns/list_zones.php?apiuser={}&apikey={}",
            self.user, self.key);
        let resp = http::get(&url, &[])
            .map_err(|e| Error::Provider(format!("durabledns list zones: {e}")))?;
        let v: Value = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Json(format!("durabledns parse: {e}")))?;
        if let Some(zones) = v.as_array() {
            for z in zones {
                let name = z.get("zone_name").or_else(|| z.get("name")).and_then(|n| n.as_str());
                if name == Some(domain) {
                    if let Some(id) = z.get("id").and_then(|i| i.as_str()).or_else(|| z.get("zone_id").and_then(|i| i.as_str())) {
                        return Ok(id.to_string());
                    }
                }
            }
        }
        Err(Error::Provider(format!("durabledns zone not found for {domain}")))
    }

    fn find_record(&self, domain: &str, name: &str, value: &str) -> Option<String> {
        let url = format!("https://api.durabledns.com/dns/list_records.php?apiuser={}&apikey={}&zonename={domain}",
            self.user, self.key);
        let resp = http::get(&url, &[]).ok()?;
        let v: Value = serde_json::from_str(&resp.body).ok()?;
        if let Some(records) = v.as_array() {
            let search_name = if name == domain { "" } else {
                let prefix = format!(".{domain}");
                name.strip_suffix(&prefix).unwrap_or(name)
            };
            for rec in records {
                let rec_type = rec.get("type").and_then(|t| t.as_str());
                let rec_name = rec.get("name").and_then(|n| n.as_str());
                let rec_content = rec.get("content").and_then(|c| c.as_str());
                if rec_type == Some("TXT") && (rec_name == Some(name) || rec_name == Some(search_name)) && rec_content == Some(value) {
                    return rec.get("id").and_then(|i| i.as_str()).or_else(|| rec.get("record_id").and_then(|i| i.as_str())).map(|s| s.to_string());
                }
            }
        }
        None
    }
}
