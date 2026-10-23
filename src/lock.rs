use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::error::Error;

pub struct Lock {
    path: PathBuf,
    _file: File,
}

impl Lock {
    pub fn acquire(home: &Path) -> Result<Self, Error> {
        std::fs::create_dir_all(home)?;
        let path = home.join(".lock");
        match create(&path) {
            Ok(file) => Ok(Lock { path, _file: file }),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if stale(&path) {
                    let _ = std::fs::remove_file(&path);
                    match create(&path) {
                        Ok(file) => Ok(Lock { path, _file: file }),
                        Err(_) => Err(contention(&path)),
                    }
                } else {
                    Err(contention(&path))
                }
            }
            Err(e) => Err(Error::Io(e)),
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn create(path: &Path) -> std::io::Result<File> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    write!(file, "{}", std::process::id())?;
    file.flush()?;
    Ok(file)
}

fn stale(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else {
        return true;
    };
    let mut pid = String::new();
    if file.read_to_string(&mut pid).is_err() {
        return false;
    }
    let Ok(pid) = pid.trim().parse::<u32>() else {
        return false;
    };
    !Path::new(&format!("/proc/{pid}")).exists()
}

fn contention(path: &Path) -> Error {
    Error::Config(format!(
        "another acmers process is running (lock: {})",
        path.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("acmers-lock-{name}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn prevents_two_holders_and_releases_on_drop() {
        let home = temp_home("contend");
        let lock = Lock::acquire(&home).unwrap();
        assert!(home.join(".lock").exists());
        assert!(Lock::acquire(&home).is_err());
        drop(lock);
        assert!(!home.join(".lock").exists());
        let again = Lock::acquire(&home).unwrap();
        drop(again);
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn recovers_stale_lock() {
        let home = temp_home("stale");
        std::fs::write(home.join(".lock"), "4294967295").unwrap();
        let lock = Lock::acquire(&home).unwrap();
        assert!(home.join(".lock").exists());
        drop(lock);
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn separate_homes_have_separate_locks() {
        let one = temp_home("one");
        let two = temp_home("two");
        let first = Lock::acquire(&one).unwrap();
        let second = Lock::acquire(&two).unwrap();
        drop(first);
        drop(second);
        let _ = std::fs::remove_dir_all(one);
        let _ = std::fs::remove_dir_all(two);
    }
}
