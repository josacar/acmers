use std::collections::HashMap;

use crate::base64;
use crate::error::Error;
use crate::http;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Mydnsjp {
    master_id: String,
    password: String,
    basic_auth: String,
}

impl DnsProvider for Mydnsjp {
    fn slug() -> &'static str {
        "mydnsjp"
    }

    fn env_vars() -> &'static [&'static str] {
        &["MYDNSJP_MasterID", "MYDNSJP_MasterPassword"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let master_id = env.get("MYDNSJP_MasterID")
            .ok_or_else(|| Error::Config("MYDNSJP_MasterID required".into()))?
            .clone();
        let password = env.get("MYDNSJP_MasterPassword")
            .ok_or_else(|| Error::Config("MYDNSJP_MasterPassword required".into()))?
            .clone();
        let creds = format!("{master_id}:{password}");
        let basic_auth = format!("Basic {}", base64::encode_std(creds.as_bytes()));
        Ok(Box::new(Mydnsjp { master_id, password, basic_auth }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let root_domain = self.get_root_domain()?;
        self.edit_record(&root_domain, value, "REGIST")
            .map_err(|e| Error::Provider(format!("MyDNS.JP add TXT for {name}: {e}")))?;
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let root_domain = match self.get_root_domain() {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };
        match self.edit_record(&root_domain, value, "DELETE") {
            Ok(()) => {}
            Err(e) => eprintln!("warning: MyDNS.JP remove TXT for {name}: {e}"),
        }
        Ok(())
    }
}

impl Mydnsjp {
    fn get_root_domain(&self) -> Result<String, Error> {
        let data = format!("MENU=100&masterid={}&masterpwd={}", self.master_id, self.password);
        let resp = http::post(
            "https://www.mydns.jp/members/",
            data.as_bytes(),
            "application/x-www-form-urlencoded",
            &[],
        ).map_err(|e| Error::Provider(format!("MyDNS.JP login: {e}")))?;
        let domain = extract_value(&resp.body, "DNSINFO[domainname]")
            .ok_or_else(|| Error::Provider("MyDNS.JP: could not find root domain".into()))?;
        Ok(domain)
    }

    fn edit_record(&self, domain: &str, value: &str, cmd: &str) -> Result<(), Error> {
        let data = format!("CERTBOT_DOMAIN={domain}&CERTBOT_VALIDATION={value}&EDIT_CMD={cmd}");
        let resp = http::post(
            "https://www.mydns.jp/directedit.html",
            data.as_bytes(),
            "application/x-www-form-urlencoded",
            &[("Authorization", &self.basic_auth)],
        ).map_err(|e| Error::Provider(format!("MyDNS.JP {cmd}: {e}")))?;
        if !resp.body.contains("OK.") {
            return Err(Error::Provider(format!("MyDNS.JP {cmd}: {}", resp.body)));
        }
        Ok(())
    }
}

fn extract_value(body: &str, key: &str) -> Option<String> {
    let needle = format!("{key}");
    let pos = body.find(&needle)?;
    let rest = &body[pos + needle.len()..];
    let val_start = rest.find("value=\"")?;
    let rest = &rest[val_start + 7..];
    let val_end = rest.find('"')?;
    Some(rest[..val_end].to_string())
}
