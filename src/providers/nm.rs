use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::helpers;
use crate::providers::{DnsProvider, ProviderResult};

const BASE_URL: &str = "https://namemaster.de/api/api.php";

pub struct Nm {
    user: String,
    sha256: String,
}

impl DnsProvider for Nm {
    fn slug() -> &'static str {
        "nm"
    }
    fn env_vars() -> &'static [&'static str] {
        &["NM_user", "NM_sha256"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let user = env
            .get("NM_user")
            .ok_or_else(|| Error::Config("NM_user required".into()))?
            .clone();
        let sha256 = env
            .get("NM_sha256")
            .ok_or_else(|| Error::Config("NM_sha256 required".into()))?
            .clone();
        Ok(Box::new(Nm { user, sha256 }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let zone = self.get_zone(name)?;

        let url = format!(
            "{BASE_URL}?User={}&Password={}&Antwort=csv&Typ=ACME&zone={}&hostname={}&TXT={}&Action=Auto&Lifetime=3600",
            helpers::url_encode_form(&self.user),
            helpers::url_encode_form(&self.sha256),
            helpers::url_encode_form(&zone),
            helpers::url_encode_form(name),
            helpers::url_encode_form(value),
        );

        let resp = http::get(&url, &[]).map_err(|e| Error::Provider(format!("nm add TXT: {e}")))?;
        if !resp.body.contains("Success") {
            return Err(Error::Provider(format!("nm add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, _name: &str, _value: &str) -> ProviderResult {
        Ok(())
    }
}

impl Nm {
    fn get_zone(&self, hostname: &str) -> Result<String, Error> {
        let url = format!(
            "{BASE_URL}?User={}&Password={}&Typ=acme&hostname={}&Action=getzone&antwort=csv",
            helpers::url_encode_form(&self.user),
            helpers::url_encode_form(&self.sha256),
            helpers::url_encode_form(hostname),
        );

        let resp = http::get(&url, &[]).map_err(|e| Error::Provider(format!("nm getzone: {e}")))?;
        if resp.body.contains("hostname not found") {
            return Err(Error::Provider(format!(
                "nm getzone: hostname not found: {hostname}"
            )));
        }
        Ok(resp.body.trim().to_string())
    }
}
