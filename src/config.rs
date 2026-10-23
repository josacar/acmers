use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::Error;

#[derive(Clone, Debug)]
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

    pub fn accounts_dir(&self) -> PathBuf {
        self.home.join("accounts")
    }

    pub fn account_file_for(&self, server: &str) -> PathBuf {
        self.accounts_dir()
            .join(format!("{}.json", server_slug(server)))
    }

    pub fn legacy_account_file(&self) -> PathBuf {
        self.home.join("account.json")
    }

    pub fn domain_dir(&self, domain: &str) -> PathBuf {
        self.home.join(domain.replace('*', "_"))
    }

    pub fn cert_file(&self, domain: &str) -> PathBuf {
        self.domain_dir(domain).join("cert.pem")
    }

    pub fn chain_file(&self, domain: &str) -> PathBuf {
        self.domain_dir(domain).join("chain.pem")
    }

    pub fn key_file(&self, domain: &str) -> PathBuf {
        self.domain_dir(domain).join("key.pem")
    }

    pub fn privkey_file(&self, domain: &str) -> PathBuf {
        self.domain_dir(domain).join("privkey.pem")
    }

    pub fn fullchain_file(&self, domain: &str) -> PathBuf {
        self.domain_dir(domain).join("fullchain.pem")
    }

    pub fn renewal_file(&self, domain: &str) -> PathBuf {
        self.domain_dir(domain).join("renewal.json")
    }

    pub fn certificate_domains(&self) -> Result<Vec<String>, Error> {
        let mut domains = Vec::new();
        if !self.home.exists() {
            return Ok(domains);
        }
        for entry in std::fs::read_dir(&self.home)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() || path.file_name().and_then(|n| n.to_str()) == Some("accounts") {
                continue;
            }
            if !path.join("cert.pem").exists() && !path.join("renewal.json").exists() {
                continue;
            }
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let stored = std::fs::read_to_string(path.join("renewal.json"))
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|v| {
                    v.get("domains")
                        .and_then(|d| d.as_array())
                        .and_then(|a| a.first())
                        .and_then(|d| d.as_str())
                        .map(|s| s.to_string())
                });
            if let Some(domain) = stored {
                domains.push(domain);
            } else {
                domains.push(dir_name.replace('_', "*"));
            }
        }
        domains.sort();
        Ok(domains)
    }
}

pub fn server_slug(server: &str) -> String {
    let without_scheme = server.split("://").nth(1).unwrap_or(server);
    without_scheme
        .trim_end_matches('/')
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn temp_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("artifact");
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!(
        ".{name}.tmp.{}.{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ))
}

pub fn set_mode(path: &Path, mode: u32) -> Result<(), Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
    Ok(())
}

pub fn write_atomic(path: &Path, data: &[u8], mode: u32) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = temp_path(path);
    let result = (|| -> Result<(), Error> {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        set_mode(&tmp, mode)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn is_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .map(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(1))
                == Some("0")
        })
        .unwrap_or(false)
}

pub fn get_home_dir() -> Result<PathBuf, Error> {
    if let Ok(dir) = std::env::var("ACMERS_HOME") {
        return Ok(PathBuf::from(dir));
    }

    if is_root() {
        return Ok(PathBuf::from("/var/lib/acmers"));
    }

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
