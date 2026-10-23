mod mock;

use std::collections::HashMap;
use std::sync::Arc;

use mock::MockServer;

use acmers::acme;
use acmers::config::Config;
use acmers::crypto;
use acmers::http;
use acmers::renewal::RenewalConfig;
use acmers::storage;

type MockHandler = Arc<
    dyn Fn(&str, &str, &[u8], &HashMap<String, String>) -> (u16, String, HashMap<String, String>)
        + Send
        + Sync,
>;

const LEAF: &str = "-----BEGIN CERTIFICATE-----\nQUJD\n-----END CERTIFICATE-----\n";
const INTERMEDIATE: &str = "-----BEGIN CERTIFICATE-----\nREVG\n-----END CERTIFICATE-----\n";

fn headers(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn json(v: serde_json::Value) -> String {
    v.to_string()
}

#[test]
fn mock_acme_issuance_writes_file_set() {
    let handler: MockHandler = Arc::new(|method, path, _body, _headers| match (method, path) {
        ("GET", "/directory") => (
            200,
            json(serde_json::json!({
                "newNonce": "https://mock.test/new-nonce",
                "newAccount": "https://mock.test/new-account",
                "newOrder": "https://mock.test/new-order",
                "revokeCert": "https://mock.test/revoke",
                "keyChange": "https://mock.test/key-change",
                "renewalInfo": "https://mock.test/renewal-info"
            })),
            HashMap::new(),
        ),
        ("HEAD", "/new-nonce") => (200, String::new(), headers(&[("Replay-Nonce", "nonce-1")])),
        ("POST", "/new-account") => (
            201,
            json(serde_json::json!({"status": "valid"})),
            headers(&[
                ("Location", "https://mock.test/account/1"),
                ("Replay-Nonce", "nonce-2"),
            ]),
        ),
        ("POST", "/new-order") => (
            201,
            json(serde_json::json!({
                "status": "pending",
                "url": "https://mock.test/order/1",
                "authorizations": ["https://mock.test/authz/1"],
                "finalize": "https://mock.test/finalize/1",
                "identifiers": [
                    {"type": "dns", "value": "example.com"},
                    {"type": "dns", "value": "www.example.com"}
                ]
            })),
            headers(&[
                ("Location", "https://mock.test/order/1"),
                ("Replay-Nonce", "nonce-3"),
            ]),
        ),
        ("POST", "/authz/1") => (
            200,
            json(serde_json::json!({
                "identifier": {"type": "dns", "value": "example.com"},
                "status": "pending",
                "challenges": [{
                    "type": "dns-01",
                    "url": "https://mock.test/chall/1",
                    "token": "token-1",
                    "status": "pending"
                }]
            })),
            headers(&[("Replay-Nonce", "nonce-4")]),
        ),
        ("POST", "/chall/1") => (
            200,
            json(serde_json::json!({"status": "valid"})),
            headers(&[("Replay-Nonce", "nonce-5")]),
        ),
        ("POST", "/finalize/1") => (
            200,
            json(serde_json::json!({
                "status": "processing",
                "url": "https://mock.test/order/1"
            })),
            headers(&[("Replay-Nonce", "nonce-6")]),
        ),
        ("POST", "/order/1") => (
            200,
            json(serde_json::json!({
                "status": "valid",
                "certificate": "https://mock.test/cert/1"
            })),
            headers(&[("Replay-Nonce", "nonce-7")]),
        ),
        ("POST", "/cert/1") => (
            200,
            format!("{LEAF}{INTERMEDIATE}"),
            headers(&[
                ("Content-Type", "application/pem-certificate-chain"),
                ("Replay-Nonce", "nonce-8"),
            ]),
        ),
        _ => (
            404,
            json(serde_json::json!({"detail": format!("unhandled {method} {path}")})),
            HashMap::new(),
        ),
    });

    let server = MockServer::new(handler);
    http::set_test_base(&server.url());

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let home =
        std::env::temp_dir().join(format!("acmers-acme-mock-{}-{nanos}", std::process::id()));
    let config = Config {
        home,
        server: "https://mock.test/directory".to_string(),
    };

    let account =
        acme::account::load_or_register(&config, "user@example.com", "https://mock.test/directory")
            .unwrap();
    assert_eq!(account.url, "https://mock.test/account/1");

    let account_key =
        crypto::load_key_from_der(&acmers::base64::decode(&account.key_b64).unwrap()).unwrap();
    let mut nonce = || Ok("nonce".to_string());

    let directory = acme::directory::fetch(&config.server).unwrap();
    let order = acme::order::create_order(
        &["example.com".to_string(), "www.example.com".to_string()],
        &account.url,
        &directory.new_order,
        &account_key,
        "nonce",
    )
    .unwrap();
    assert_eq!(order.url, "https://mock.test/order/1");

    let auths = acme::challenge::get_authorizations(
        &account.url,
        &order.authorizations,
        &account_key,
        &mut nonce,
    )
    .unwrap();
    assert_eq!(auths.len(), 1);
    let challenge = &auths[0].challenges[0];
    assert_eq!(challenge.typ, "dns-01");

    acme::challenge::respond_to_challenge(&challenge.url, &account.url, &account_key, &mut nonce)
        .unwrap();
    acme::challenge::poll_challenge(&challenge.url, &account.url, &account_key, &mut nonce)
        .unwrap();

    let cert_key = crypto::generate_p256_key().unwrap();
    let csr = crypto::create_csr(
        &["example.com".to_string(), "www.example.com".to_string()],
        &cert_key.pkcs8_bytes,
    )
    .unwrap();
    let cert_url = acme::order::finalize_order(
        &csr,
        &order.finalize,
        &account.url,
        &account_key,
        &mut nonce,
    )
    .unwrap();
    let pem =
        acme::order::download_cert(&cert_url, &account.url, &account_key, &mut nonce).unwrap();

    let rc = RenewalConfig::new(
        vec!["example.com".to_string(), "www.example.com".to_string()],
        "cf".to_string(),
        "user@example.com".to_string(),
        "https://mock.test/directory".to_string(),
    );
    storage::save_certificate(&config, &rc, &pem, &cert_key.pkcs8_bytes).unwrap();

    assert_eq!(
        std::fs::read_to_string(config.cert_file("example.com")).unwrap(),
        LEAF
    );
    assert_eq!(
        std::fs::read_to_string(config.chain_file("example.com")).unwrap(),
        INTERMEDIATE
    );
    assert_eq!(
        std::fs::read_to_string(config.fullchain_file("example.com")).unwrap(),
        format!("{LEAF}{INTERMEDIATE}")
    );
    assert!(
        std::fs::read_to_string(config.privkey_file("example.com"))
            .unwrap()
            .starts_with("-----BEGIN PRIVATE KEY-----")
    );

    http::clear_test_base();
}
