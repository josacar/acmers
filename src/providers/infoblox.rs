use std::collections::HashMap;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Infoblox {
    basic_auth: String,
    server: String,
    view_encoded: String,
}

fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 3);
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

impl Infoblox {
    fn wapi_base(&self) -> String {
        format!("https://{}/wapi/v2.2.2", self.server)
    }

    fn auth_headers(&self) -> Vec<(&str, &str)> {
        vec![
            ("Accept-Language", "en-US"),
            ("Authorization", &self.basic_auth),
        ]
    }

    fn find_record_ref(&self, fulldomain: &str, txtvalue: &str) -> Result<Option<String>, Error> {
        let url = format!(
            "{}/record:txt?name={}&text={}&view={}",
            self.wapi_base(),
            percent_encode(fulldomain),
            percent_encode(txtvalue),
            self.view_encoded,
        );
        let hdrs = self.auth_headers();
        let resp = http::get(&url, &hdrs)
            .map_err(|e| Error::Provider(format!("infoblox find record: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("infoblox find record: HTTP {} {}", resp.status, resp.body)));
        }
        let records: Vec<serde_json::Value> = serde_json::from_str(&resp.body)
            .map_err(|e| Error::Provider(format!("infoblox find record: {e}")))?;
        for record in &records {
            if let Some(r#ref) = record.get("_ref").and_then(|v| v.as_str()) {
                if r#ref.contains("record:txt/") && r#ref.contains(&self.view_encoded) {
                    return Ok(Some(r#ref.to_string()));
                }
            }
        }
        Ok(None)
    }
}

impl DnsProvider for Infoblox {
    fn slug() -> &'static str {
        "infoblox"
    }

    fn env_vars() -> &'static [&'static str] {
        &["Infoblox_Creds", "Infoblox_Server", "Infoblox_View"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let creds = env.get("Infoblox_Creds")
            .ok_or_else(|| Error::Config("Infoblox_Creds required".into()))?;
        let server = env.get("Infoblox_Server")
            .ok_or_else(|| Error::Config("Infoblox_Server required".into()))?;
        let view = env.get("Infoblox_View").map(|s| s.as_str()).unwrap_or("default");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        let view_encoded = percent_encode(view);
        Ok(Box::new(Infoblox { basic_auth, server: server.clone(), view_encoded }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let url = format!(
            "{}/record:txt?name={}&text={}&view={}",
            self.wapi_base(),
            percent_encode(name),
            percent_encode(value),
            self.view_encoded,
        );
        let hdrs = self.auth_headers();
        let resp = http::post(&url, b"", "application/json", &hdrs)
            .map_err(|e| Error::Provider(format!("infoblox add TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("infoblox add TXT: HTTP {} {}", resp.status, resp.body)));
        }
        if let Some(r#ref) = serde_json::from_str::<serde_json::Value>(&resp.body)
            .ok()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
        {
            if r#ref.contains("record:txt/") && r#ref.contains(&self.view_encoded) {
                return Ok(());
            }
        }
        Err(Error::Provider(format!("infoblox add TXT: unexpected response: {}", resp.body)))
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let r#ref = match self.find_record_ref(name, value) {
            Ok(Some(r)) => r,
            Ok(None) => return Ok(()),
            Err(e) => {
                eprintln!("warning: infoblox cleanup: {e}");
                return Ok(());
            }
        };
        let url = format!("{}/{}", self.wapi_base(), r#ref);
        let hdrs = self.auth_headers();
        let resp = http::delete(&url, &hdrs)
            .map_err(|e| Error::Provider(format!("infoblox remove TXT: {e}")))?;
        if resp.status >= 400 {
            return Err(Error::Provider(format!("infoblox remove TXT: HTTP {} {}", resp.status, resp.body)));
        }
        Ok(())
    }
}
