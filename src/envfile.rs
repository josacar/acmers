use std::collections::HashMap;
use std::path::Path;

use crate::error::Error;

pub fn load(path: &Path) -> Result<HashMap<String, String>, Error> {
    let data = std::fs::read_to_string(path)
        .map_err(|e| Error::Config(format!("read env file {}: {e}", path.display())))?;
    parse(&data, path)
}

pub fn parse(data: &str, path: &Path) -> Result<HashMap<String, String>, Error> {
    let mut map = HashMap::new();
    for (index, raw) in data.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((key, value)) = line.split_once('=') else {
            return Err(Error::Config(format!(
                "invalid env file {} at line {}: expected KEY=VALUE",
                path.display(),
                index + 1
            )));
        };
        let key = key.trim();
        if key.is_empty() || !valid_key(key) {
            return Err(Error::Config(format!(
                "invalid env file {} at line {}: invalid variable name",
                path.display(),
                index + 1
            )));
        }
        map.insert(key.to_string(), unquote(value.trim()).to_string());
    }
    Ok(map)
}

pub fn merge(
    path: Option<&Path>,
    explicit: &HashMap<String, String>,
) -> Result<HashMap<String, String>, Error> {
    let mut merged = match path {
        Some(path) => load(path)?,
        None => HashMap::new(),
    };
    for (key, value) in explicit {
        merged.insert(key.clone(), value.clone());
    }
    Ok(merged)
}

fn valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn unquote(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &value[1..value.len() - 1];
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path() -> &'static Path {
        Path::new("/etc/acmers/env")
    }

    #[test]
    fn parses_comments_blank_lines_and_quotes() {
        let map = parse(
            "# comment\n\nCF_Token=\"abc def\"\nCF_Key='key'\nexport CF_Email=user@example.com\n",
            path(),
        )
        .unwrap();
        assert_eq!(map.get("CF_Token").unwrap(), "abc def");
        assert_eq!(map.get("CF_Key").unwrap(), "key");
        assert_eq!(map.get("CF_Email").unwrap(), "user@example.com");
    }

    #[test]
    fn rejects_malformed_line_without_leaking_content() {
        let err = parse("CF_Token=ok\nsecret-value\n", path())
            .unwrap_err()
            .to_string();
        assert!(err.contains("line 2"));
        assert!(!err.contains("secret-value"));
    }

    #[test]
    fn rejects_invalid_names() {
        assert!(parse("1BAD=x\n", path()).is_err());
        assert!(parse("=x\n", path()).is_err());
    }

    #[test]
    fn merge_prefers_explicit_values() {
        let dir = std::env::temp_dir().join(format!("acmers-env-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("env");
        std::fs::write(&file, "CF_Token=file\nCF_Key=file\n").unwrap();
        let mut explicit = HashMap::new();
        explicit.insert("CF_Token".to_string(), "cli".to_string());
        let merged = merge(Some(&file), &explicit).unwrap();
        assert_eq!(merged.get("CF_Token").unwrap(), "cli");
        assert_eq!(merged.get("CF_Key").unwrap(), "file");
        let _ = std::fs::remove_dir_all(dir);
    }
}
