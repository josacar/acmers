use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::Error;

#[derive(Clone)]
pub struct Config {
    pub home: PathBuf,
    pub server: String,
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        let home = get_home_dir()?;
        std::fs::create_dir_all(&home)?;
        Ok(Config {
            home,
            server: "https://acme-v02.api.letsencrypt.org/directory".to_string(),
        })
    }

    pub fn account_file(&self) -> PathBuf {
        self.home.join("account.json")
    }

    pub fn domain_dir(&self, domain: &str) -> PathBuf {
        self.home.join(domain.replace('*', "_"))
    }

    pub fn cert_file(&self, domain: &str) -> PathBuf {
        self.domain_dir(domain).join("cert.pem")
    }

    pub fn key_file(&self, domain: &str) -> PathBuf {
        self.domain_dir(domain).join("key.pem")
    }

    pub fn fullchain_file(&self, domain: &str) -> PathBuf {
        self.domain_dir(domain).join("fullchain.pem")
    }
}

pub fn get_home_dir() -> Result<PathBuf, Error> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| Error::Config("cannot find home directory".into()))?;
    Ok(PathBuf::from(home).join(".acmers"))
}

pub fn read_env_vars(names: &[&str]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for name in names {
        if let Ok(val) = std::env::var(name) {
            map.insert(name.to_string(), val);
        }
    }
    map
}
