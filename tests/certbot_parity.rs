use std::path::PathBuf;

use acmers::acme::account;
use acmers::config::{self, Config};
use acmers::crypto;
use acmers::deploy;
use acmers::renewal::RenewalConfig;
use acmers::storage;

const PRODUCTION: &str = "https://acme-v02.api.letsencrypt.org/directory";
const STAGING: &str = "https://acme-staging-v02.api.letsencrypt.org/directory";

fn temp_dir(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir =
        std::env::temp_dir().join(format!("acmers-test-{name}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn test_config(name: &str) -> Config {
    Config {
        home: temp_dir(name),
        server: PRODUCTION.to_string(),
    }
}

#[cfg(unix)]
fn mode(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn pkcs8_pem_round_trip() {
    let key = crypto::generate_p256_key().unwrap();
    let pem = crypto::pkcs8_to_pem(&key.pkcs8_bytes);
    assert!(pem.starts_with("-----BEGIN PRIVATE KEY-----\n"));
    assert!(pem.ends_with("-----END PRIVATE KEY-----\n"));
    let body: String = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    assert!(pem.lines().all(|line| line.len() <= 64));
    assert_eq!(acmers::base64::decode_std(&body).unwrap(), key.pkcs8_bytes);
}

#[test]
fn splits_leaf_and_chain() {
    let leaf = "-----BEGIN CERTIFICATE-----\nQUJD\n-----END CERTIFICATE-----\n";
    let intermediate = "-----BEGIN CERTIFICATE-----\nREVG\n-----END CERTIFICATE-----\n";
    let chain = acmers::acme::order::split_cert_chain(&format!("{leaf}{intermediate}"));
    assert_eq!(chain.leaf, leaf);
    assert_eq!(chain.chain, intermediate);
    assert_eq!(chain.fullchain, format!("{leaf}{intermediate}"));
}

#[test]
fn split_handles_crlf_and_single_certificate() {
    let leaf = "-----BEGIN CERTIFICATE-----\r\nQUJD\r\n-----END CERTIFICATE-----\r\n";
    let chain = acmers::acme::order::split_cert_chain(leaf);
    assert!(chain.leaf.contains("QUJD"));
    assert!(chain.chain.is_empty());
    assert_eq!(chain.fullchain, chain.leaf);
}

#[test]
fn atomic_write_sets_mode_and_replaces_content() {
    let dir = temp_dir("atomic");
    let path = dir.join("artifact.pem");
    config::write_atomic(&path, b"first", 0o644).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "first");
    config::write_atomic(&path, b"second", 0o600).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
    #[cfg(unix)]
    assert_eq!(mode(&path), 0o600);
    let leftover: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
        .collect();
    assert!(leftover.is_empty());
}

#[test]
fn legacy_renewal_defaults_domains() {
    let v = serde_json::json!({
        "email": "user@example.com",
        "provider": "cf",
        "test": false,
        "standalone": false,
        "server": PRODUCTION,
    });
    let rc = RenewalConfig::from_value(&v, "example.com");
    assert_eq!(rc.domains, vec!["example.com"]);
    assert_eq!(rc.challenge, "dns-01");
    assert_eq!(rc.key_type, "ecdsa-p256");
    assert_eq!(rc.dnssleep, 10);
    assert_eq!(rc.dns_timeout, 120);
}

#[test]
fn renewal_round_trip_preserves_sans_and_hooks() {
    let config = test_config("renewal");
    let mut rc = RenewalConfig::new(
        vec![
            "example.com".to_string(),
            "www.example.com".to_string(),
            "*.example.com".to_string(),
        ],
        "cf".to_string(),
        "user@example.com".to_string(),
        PRODUCTION.to_string(),
    );
    rc.staging = true;
    rc.dnssleep = 30;
    rc.dns_timeout = 90;
    rc.dns_servers = vec!["1.1.1.1".to_string()];
    rc.preferred_chain = Some("ISRG Root X1".to_string());
    rc.fullchain_path = Some("/etc/ssl/example/fullchain.pem".to_string());
    rc.reloadcmd = Some("systemctl reload nginx".to_string());
    rc.save(&config).unwrap();

    let loaded = RenewalConfig::load(&config, "example.com").unwrap();
    assert_eq!(loaded.domains, rc.domains);
    assert!(loaded.staging);
    assert_eq!(loaded.dnssleep, 30);
    assert_eq!(loaded.dns_timeout, 90);
    assert_eq!(loaded.dns_servers, vec!["1.1.1.1"]);
    assert_eq!(loaded.preferred_chain.as_deref(), Some("ISRG Root X1"));
    assert_eq!(loaded.reloadcmd.as_deref(), Some("systemctl reload nginx"));
}

#[test]
fn lists_stored_certificates() {
    let config = test_config("list");
    let rc = RenewalConfig::new(
        vec!["example.com".to_string()],
        "cf".to_string(),
        "user@example.com".to_string(),
        PRODUCTION.to_string(),
    );
    rc.save(&config).unwrap();
    config::write_atomic(&config.cert_file("example.com"), b"not a cert", 0o644).unwrap();
    let domains = config.certificate_domains().unwrap();
    assert_eq!(domains, vec!["example.com"]);
}

#[test]
fn account_slugs_are_distinct() {
    let prod = config::server_slug(PRODUCTION);
    let staging = config::server_slug(STAGING);
    assert_ne!(prod, staging);
    assert!(prod.contains("acme-v02.api.letsencrypt.org"));
}

#[test]
fn imports_legacy_account_per_server() {
    let config = test_config("account");
    let legacy = config.legacy_account_file();
    let body = serde_json::json!({
        "url": "https://acme-v02.api.letsencrypt.org/acme/acct/1",
        "key": acmers::base64::encode_no_pad(&[1, 2, 3]),
        "jwk_thumbprint": "thumb",
        "server": PRODUCTION,
        "email": "user@example.com",
    });
    config::write_atomic(&legacy, body.to_string().as_bytes(), 0o600).unwrap();

    let account = account::load(&config, PRODUCTION).unwrap();
    assert_eq!(
        account.url,
        "https://acme-v02.api.letsencrypt.org/acme/acct/1"
    );
    assert_eq!(account.email, "user@example.com");
    let imported = config.account_file_for(PRODUCTION);
    assert!(imported.exists());
    #[cfg(unix)]
    assert_eq!(mode(&imported), 0o600);

    assert!(account::load(&config, STAGING).is_none());
}

#[test]
fn deploys_files_and_exports_hook_environment() {
    let config = test_config("deploy");
    config::write_atomic(&config.cert_file("example.com"), b"leaf", 0o644).unwrap();
    config::write_atomic(&config.chain_file("example.com"), b"chain", 0o644).unwrap();
    config::write_atomic(&config.fullchain_file("example.com"), b"fullchain", 0o644).unwrap();
    config::write_atomic(&config.privkey_file("example.com"), b"key", 0o600).unwrap();

    let out = temp_dir("deploy-out");
    let env_file = out.join("hook.env");
    let mut rc = RenewalConfig::new(
        vec!["example.com".to_string(), "www.example.com".to_string()],
        "cf".to_string(),
        "user@example.com".to_string(),
        PRODUCTION.to_string(),
    );
    rc.cert_path = Some(out.join("cert.pem").to_string_lossy().to_string());
    rc.key_path = Some(out.join("privkey.pem").to_string_lossy().to_string());
    rc.fullchain_path = Some(out.join("fullchain.pem").to_string_lossy().to_string());
    rc.chain_path = Some(out.join("chain.pem").to_string_lossy().to_string());
    rc.reloadcmd = Some(format!(
        "printf '%s|%s' \"$RENEWED_LINEAGE\" \"$RENEWED_DOMAINS\" > {}",
        env_file.display()
    ));

    deploy::deploy_certificate(&config, &rc).unwrap();
    assert_eq!(
        std::fs::read_to_string(out.join("cert.pem")).unwrap(),
        "leaf"
    );
    assert_eq!(
        std::fs::read_to_string(out.join("privkey.pem")).unwrap(),
        "key"
    );
    #[cfg(unix)]
    {
        assert_eq!(mode(&out.join("cert.pem")), 0o644);
        assert_eq!(mode(&out.join("privkey.pem")), 0o600);
    }
    let env = std::fs::read_to_string(&env_file).unwrap();
    assert!(env.contains("example.com www.example.com"));
    assert!(
        env.contains(
            &config
                .domain_dir("example.com")
                .to_string_lossy()
                .to_string()
        )
    );
}

#[test]
fn hook_failure_is_reported() {
    let config = test_config("hook-fail");
    let mut rc = RenewalConfig::new(
        vec!["example.com".to_string()],
        "cf".to_string(),
        "user@example.com".to_string(),
        PRODUCTION.to_string(),
    );
    rc.reloadcmd = Some("exit 7".to_string());
    assert!(deploy::deploy_certificate(&config, &rc).is_err());
}

#[test]
fn storage_writes_complete_file_set() {
    let config = test_config("storage");
    let key = crypto::generate_p256_key().unwrap();
    let leaf = "-----BEGIN CERTIFICATE-----\nQUJD\n-----END CERTIFICATE-----\n";
    let intermediate = "-----BEGIN CERTIFICATE-----\nREVG\n-----END CERTIFICATE-----\n";
    let rc = RenewalConfig::new(
        vec!["example.com".to_string(), "www.example.com".to_string()],
        "cf".to_string(),
        "user@example.com".to_string(),
        PRODUCTION.to_string(),
    );

    storage::save_certificate(
        &config,
        &rc,
        &format!("{leaf}{intermediate}"),
        &key.pkcs8_bytes,
    )
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(config.cert_file("example.com")).unwrap(),
        leaf
    );
    assert_eq!(
        std::fs::read_to_string(config.chain_file("example.com")).unwrap(),
        intermediate
    );
    assert_eq!(
        std::fs::read_to_string(config.fullchain_file("example.com")).unwrap(),
        format!("{leaf}{intermediate}")
    );
    let privkey = std::fs::read_to_string(config.privkey_file("example.com")).unwrap();
    assert!(privkey.starts_with("-----BEGIN PRIVATE KEY-----"));
    assert_eq!(
        std::fs::read_to_string(config.key_file("example.com")).unwrap(),
        privkey
    );
    assert!(config.renewal_file("example.com").exists());
    #[cfg(unix)]
    {
        assert_eq!(mode(&config.privkey_file("example.com")), 0o600);
        assert_eq!(mode(&config.key_file("example.com")), 0o600);
        assert_eq!(mode(&config.cert_file("example.com")), 0o644);
    }
}
