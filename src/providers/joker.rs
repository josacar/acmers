use std::collections::HashMap;

use crate::error::Error;
use crate::http;
use crate::providers::helpers;
use crate::providers::{DnsProvider, ProviderResult};

const JOKER_API: &str = "https://svc.joker.com/nic/replace";

pub struct Joker {
    username: String,
    password: String,
}

impl DnsProvider for Joker {
    fn slug() -> &'static str {
        "joker"
    }

    fn env_vars() -> &'static [&'static str] {
        &["JOKER_USERNAME", "JOKER_PASSWORD"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let username = env
            .get("JOKER_USERNAME")
            .ok_or_else(|| Error::Config("JOKER_USERNAME required".into()))?
            .clone();
        let password = env
            .get("JOKER_PASSWORD")
            .ok_or_else(|| Error::Config("JOKER_PASSWORD required".into()))?
            .clone();
        Ok(Box::new(Joker { username, password }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let (zone, label) = self.get_root(name)?;
        let form = format!(
            "username={}&password={}&zone={}&label={}&type=TXT&value={}",
            helpers::url_encode_form(&self.username),
            helpers::url_encode_form(&self.password),
            helpers::url_encode_form(&zone),
            helpers::url_encode_form(&label),
            helpers::url_encode_form(value),
        );
        let resp = http::post(
            JOKER_API,
            form.as_bytes(),
            "application/x-www-form-urlencoded",
            &[],
        )
        .map_err(|e| Error::Provider(format!("joker add TXT: {e}")))?;
        if !resp.body.starts_with("OK") {
            return Err(Error::Provider(format!("joker add TXT: {}", resp.body)));
        }
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, _value: &str) -> ProviderResult {
        let (zone, label) = self.get_root(name)?;
        let form = format!(
            "username={}&password={}&zone={}&label={}&type=TXT&value=",
            helpers::url_encode_form(&self.username),
            helpers::url_encode_form(&self.password),
            helpers::url_encode_form(&zone),
            helpers::url_encode_form(&label),
        );
        let resp = http::post(
            JOKER_API,
            form.as_bytes(),
            "application/x-www-form-urlencoded",
            &[],
        )
        .map_err(|e| Error::Provider(format!("joker remove TXT: {e}")))?;
        if !resp.body.starts_with("OK") {
            eprintln!("warning: joker remove TXT: {}", resp.body);
        }
        Ok(())
    }
}

impl Joker {
    fn get_root(&self, fulldomain: &str) -> Result<(String, String), Error> {
        let parts: Vec<&str> = fulldomain.split('.').collect();
        for i in 0..parts.len() {
            let candidate = parts[i..].join(".");
            if candidate.is_empty() {
                continue;
            }
            let form = format!(
                "username={}&password={}&zone={}&label=jokerTXTUpdateTest&type=TXT&value=",
                helpers::url_encode_form(&self.username),
                helpers::url_encode_form(&self.password),
                helpers::url_encode_form(&candidate),
            );
            if let Ok(resp) = http::post(
                JOKER_API,
                form.as_bytes(),
                "application/x-www-form-urlencoded",
                &[],
            ) && resp.body.starts_with("OK")
            {
                let sub_domain = if i == 0 {
                    String::new()
                } else {
                    parts[..i].join(".")
                };
                return Ok((candidate, sub_domain));
            }
        }
        Err(Error::Provider(format!(
            "joker: root domain not found for {fulldomain}"
        )))
    }
}
