use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct Maradns {
    zone_file: String,
    _pid_path: String,
}

impl DnsProvider for Maradns {
    fn slug() -> &'static str {
        "maradns"
    }

    fn env_vars() -> &'static [&'static str] {
        &["MARA_ZONE_FILE", "MARA_DUENDE_PID_PATH"]
    }

    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let zone_file = env
            .get("MARA_ZONE_FILE")
            .ok_or_else(|| Error::Config("MARA_ZONE_FILE required".into()))?
            .clone();
        let pid_path = env
            .get("MARA_DUENDE_PID_PATH")
            .ok_or_else(|| Error::Config("MARA_DUENDE_PID_PATH required".into()))?
            .clone();
        Ok(Box::new(Maradns {
            zone_file,
            _pid_path: pid_path,
        }))
    }

    fn add_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let path = Path::new(&self.zone_file);
        if !path.exists() {
            return Err(Error::Provider(format!(
                "zone file not found: {}",
                self.zone_file
            )));
        }
        let line = format!("{}. TXT '{}' ~\n", name, value);
        fs::OpenOptions::new()
            .append(true)
            .open(&self.zone_file)
            .map_err(|e| Error::Provider(format!("open zone file for append: {e}")))?
            .write_all(line.as_bytes())
            .map_err(|e| Error::Provider(format!("write to zone file: {e}")))?;
        eprintln!(
            "maradns: appended TXT record to {}. User must reload maradns (e.g. send SIGHUP to the PID in {}).",
            self.zone_file, self._pid_path
        );
        Ok(())
    }

    fn remove_txt(&self, _domain: &str, name: &str, value: &str) -> ProviderResult {
        let path = Path::new(&self.zone_file);
        if !path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(&self.zone_file)
            .map_err(|e| Error::Provider(format!("read zone file: {e}")))?;
        let prefix = format!("{}.", name);
        let target = format!("TXT '{}' ~", value);
        let mut removed = false;
        let new_content: String = content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with(&prefix) && trimmed.contains(&target) {
                    removed = true;
                    false
                } else {
                    true
                }
            })
            .collect::<Vec<&str>>()
            .join("\n");
        let new_content = if content.ends_with('\n') && !new_content.is_empty() {
            format!("{}\n", new_content)
        } else {
            new_content
        };
        if removed {
            fs::write(&self.zone_file, new_content)
                .map_err(|e| Error::Provider(format!("write zone file: {e}")))?;
            eprintln!(
                "maradns: removed TXT record from {}. User must reload maradns (e.g. send SIGHUP to the PID in {}).",
                self.zone_file, self._pid_path
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_env(zone_file: &str, pid_path: &str) -> HashMap<String, String> {
        let mut env = HashMap::new();
        env.insert("MARA_ZONE_FILE".into(), zone_file.into());
        env.insert("MARA_DUENDE_PID_PATH".into(), pid_path.into());
        env
    }

    #[test]
    fn test_new_missing_zone_file() {
        let mut env = HashMap::new();
        env.insert("MARA_DUENDE_PID_PATH".into(), "/tmp/test.pid".into());
        let result = Maradns::new(&env);
        assert!(result.is_err());
    }

    #[test]
    fn test_new_missing_pid_path() {
        let mut env = HashMap::new();
        env.insert("MARA_ZONE_FILE".into(), "/tmp/test.zone".into());
        let result = Maradns::new(&env);
        assert!(result.is_err());
    }

    #[test]
    fn test_new_success() {
        let env = make_env("/tmp/test.zone", "/tmp/test.pid");
        let result = Maradns::new(&env);
        assert!(result.is_ok());
    }

    #[test]
    fn test_add_txt_appends_line() {
        let dir = std::env::temp_dir().join("maradns_test_add");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let zone_path = dir.join("db.example.com");
        fs::write(&zone_path, "example.com. SOA ns1.example.com. admin.example.com. ~\n").unwrap();

        let m = Maradns {
            zone_file: zone_path.to_str().unwrap().into(),
            _pid_path: "/tmp/dummy.pid".into(),
        };

        let result = m.add_txt("example.com", "_acme-challenge.example.com", "testtoken123");
        assert!(result.is_ok());

        let content = fs::read_to_string(&zone_path).unwrap();
        assert!(content.contains("_acme-challenge.example.com. TXT 'testtoken123' ~"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_add_txt_preserves_existing_content() {
        let dir = std::env::temp_dir().join("maradns_test_preserve");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let zone_path = dir.join("db.example.com");
        let original = "example.com. SOA ns1.example.com. admin.example.com. ~\nexample.com. NS ns1.example.com. ~\n";
        fs::write(&zone_path, original).unwrap();

        let m = Maradns {
            zone_file: zone_path.to_str().unwrap().into(),
            _pid_path: "/tmp/dummy.pid".into(),
        };

        m.add_txt("example.com", "_acme-challenge.example.com", "token1").unwrap();

        let content = fs::read_to_string(&zone_path).unwrap();
        assert!(content.contains("example.com. SOA ns1.example.com. admin.example.com. ~"));
        assert!(content.contains("example.com. NS ns1.example.com. ~"));
        assert!(content.contains("_acme-challenge.example.com. TXT 'token1' ~"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_add_txt_zone_file_not_found() {
        let m = Maradns {
            zone_file: "/tmp/nonexistent_maradns_zone_file_12345".into(),
            _pid_path: "/tmp/dummy.pid".into(),
        };
        let result = m.add_txt("example.com", "_acme-challenge.example.com", "token");
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_txt_removes_matching_line() {
        let dir = std::env::temp_dir().join("maradns_test_rm");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let zone_path = dir.join("db.example.com");
        let content = "example.com. SOA ns1.example.com. admin.example.com. ~\n\
                        _acme-challenge.example.com. TXT 'testtoken123' ~\n\
                        example.com. NS ns1.example.com. ~\n";
        fs::write(&zone_path, content).unwrap();

        let m = Maradns {
            zone_file: zone_path.to_str().unwrap().into(),
            _pid_path: "/tmp/dummy.pid".into(),
        };

        let result = m.remove_txt("example.com", "_acme-challenge.example.com", "testtoken123");
        assert!(result.is_ok());

        let new_content = fs::read_to_string(&zone_path).unwrap();
        assert!(!new_content.contains("_acme-challenge.example.com. TXT 'testtoken123' ~"));
        assert!(new_content.contains("example.com. SOA ns1.example.com. admin.example.com. ~"));
        assert!(new_content.contains("example.com. NS ns1.example.com. ~"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_remove_txt_idempotent_no_match() {
        let dir = std::env::temp_dir().join("maradns_test_idem");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let zone_path = dir.join("db.example.com");
        let content = "example.com. SOA ns1.example.com. admin.example.com. ~\n";
        fs::write(&zone_path, content).unwrap();

        let m = Maradns {
            zone_file: zone_path.to_str().unwrap().into(),
            _pid_path: "/tmp/dummy.pid".into(),
        };

        let result = m.remove_txt("example.com", "_acme-challenge.example.com", "nonexistent");
        assert!(result.is_ok());

        let new_content = fs::read_to_string(&zone_path).unwrap();
        assert_eq!(new_content, content);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_remove_txt_zone_file_not_found() {
        let m = Maradns {
            zone_file: "/tmp/nonexistent_maradns_zone_file_12345".into(),
            _pid_path: "/tmp/dummy.pid".into(),
        };
        let result = m.remove_txt("example.com", "_acme-challenge.example.com", "token");
        assert!(result.is_ok());
    }

    #[test]
    fn test_remove_txt_only_removes_exact_match() {
        let dir = std::env::temp_dir().join("maradns_test_exact");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let zone_path = dir.join("db.example.com");
        let content = "_acme-challenge.example.com. TXT 'token_A' ~\n\
                        _acme-challenge.example.com. TXT 'token_B' ~\n\
                        _acme-challenge.www.example.com. TXT 'token_A' ~\n";
        fs::write(&zone_path, content).unwrap();

        let m = Maradns {
            zone_file: zone_path.to_str().unwrap().into(),
            _pid_path: "/tmp/dummy.pid".into(),
        };

        m.remove_txt("example.com", "_acme-challenge.example.com", "token_A").unwrap();

        let new_content = fs::read_to_string(&zone_path).unwrap();
        assert!(!new_content.contains("_acme-challenge.example.com. TXT 'token_A' ~"));
        assert!(new_content.contains("_acme-challenge.example.com. TXT 'token_B' ~"));
        assert!(new_content.contains("_acme-challenge.www.example.com. TXT 'token_A' ~"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_add_then_remove_roundtrip() {
        let dir = std::env::temp_dir().join("maradns_test_roundtrip");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let zone_path = dir.join("db.example.com");
        let original = "example.com. SOA ns1.example.com. admin.example.com. ~\n";
        fs::write(&zone_path, original).unwrap();

        let m = Maradns {
            zone_file: zone_path.to_str().unwrap().into(),
            _pid_path: "/tmp/dummy.pid".into(),
        };

        m.add_txt("example.com", "_acme-challenge.example.com", "roundtrip_token").unwrap();
        let after_add = fs::read_to_string(&zone_path).unwrap();
        assert!(after_add.contains("_acme-challenge.example.com. TXT 'roundtrip_token' ~"));

        m.remove_txt("example.com", "_acme-challenge.example.com", "roundtrip_token").unwrap();
        let after_rm = fs::read_to_string(&zone_path).unwrap();
        assert!(!after_rm.contains("roundtrip_token"));
        assert!(after_rm.contains("example.com. SOA ns1.example.com. admin.example.com. ~"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_add_multiple_records() {
        let dir = std::env::temp_dir().join("maradns_test_multi");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let zone_path = dir.join("db.example.com");
        fs::write(&zone_path, "example.com. SOA ns1.example.com. admin.example.com. ~\n").unwrap();

        let m = Maradns {
            zone_file: zone_path.to_str().unwrap().into(),
            _pid_path: "/tmp/dummy.pid".into(),
        };

        m.add_txt("example.com", "_acme-challenge.example.com", "token1").unwrap();
        m.add_txt("example.com", "_acme-challenge.www.example.com", "token2").unwrap();

        let content = fs::read_to_string(&zone_path).unwrap();
        assert!(content.contains("_acme-challenge.example.com. TXT 'token1' ~"));
        assert!(content.contains("_acme-challenge.www.example.com. TXT 'token2' ~"));

        let _ = fs::remove_dir_all(&dir);
    }
}
