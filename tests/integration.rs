use serde_json::json;

#[test]
fn test_base64_encode_decode() {
    let input = b"hello world";
    let encoded = acmers::base64::encode_no_pad(input);
    let decoded = acmers::base64::decode(&encoded).unwrap();
    assert_eq!(decoded, input);
}

#[test]
fn test_base64_url_safe() {
    let encoded = acmers::base64::encode_no_pad(b"\xff\xff\xff");
    assert!(!encoded.contains('+'));
    assert!(!encoded.contains('/'));
    assert!(!encoded.contains('='));
}

#[test]
fn test_base64_std() {
    let encoded = acmers::base64::encode_std(b"test");
    assert!(!encoded.contains('-'));
    assert!(!encoded.contains('_'));
}

#[test]
fn test_json_get_string() {
    let v: serde_json::Value = json!({"foo": {"bar": "baz"}});
    let s = acmers::json::get_string(&v, &["foo", "bar"]);
    assert_eq!(s, Some("baz"));

    let missing = acmers::json::get_string(&v, &["foo", "nope"]);
    assert_eq!(missing, None);
}

#[test]
fn test_json_get_string_required() {
    let v: serde_json::Value = json!({"key": "value"});
    let s = acmers::json::get_string_required(&v, &["key"]).unwrap();
    assert_eq!(s, "value");

    let result = acmers::json::get_string_required(&v, &["missing"]);
    assert!(result.is_err());
}

#[test]
fn test_json_get_array() {
    let v: serde_json::Value = json!({"items": [1, 2, 3]});
    let arr = acmers::json::get_array_required(&v, &["items"]).unwrap();
    assert_eq!(arr.len(), 3);
}

#[test]
fn test_json_get_value() {
    let v: serde_json::Value = json!({"nested": {"deep": 42}});
    let val = acmers::json::get_value_required(&v, &["nested", "deep"]).unwrap();
    assert_eq!(val.as_i64().unwrap(), 42);
}

#[test]
fn test_key_generation() {
    let ak = acmers::crypto::generate_p256_key().unwrap();
    assert_eq!(ak.pkcs8_bytes.len(), 138);
    assert_eq!(ak.jwk["kty"].as_str().unwrap(), "EC");
    assert_eq!(ak.jwk["crv"].as_str().unwrap(), "P-256");
}

#[test]
fn test_jwk_thumbprint() {
    let jwk = json!({"crv": "P-256", "kty": "EC", "x": "abc", "y": "def"});
    let thumb = acmers::crypto::jwk_thumbprint(&jwk);
    assert_eq!(thumb.len(), 43);
}

#[test]
fn test_jws_sign_verify() {
    let ak = acmers::crypto::generate_p256_key().unwrap();
    let payload = b"test payload";
    let jws = acmers::crypto::sign_jws(
        payload,
        &ak.key_pair,
        &acmers::crypto::KidOrJwk::Kid("https://test/account".into()),
        "test-nonce",
        "https://test/url",
    )
    .unwrap();

    assert!(jws["protected"].as_str().is_some());
    assert!(jws["payload"].as_str().is_some());
    assert!(jws["signature"].as_str().is_some());
}

#[test]
fn test_csr_creation() {
    let ak = acmers::crypto::generate_p256_key().unwrap();
    let domains = vec!["example.com".to_string(), "www.example.com".to_string()];
    let csr = acmers::crypto::create_csr(&domains, &ak.pkcs8_bytes).unwrap();
    assert!(csr.len() > 100);
}

#[test]
fn test_config_home_dir() {
    let config = acmers::config::Config::load().unwrap();
    assert!(config.home.ends_with(".acmers"));
}

#[test]
fn test_dns_txt_value() {
    let value = acmers::acme::account::dns_txt_value("token", "thumbprint");
    assert_eq!(value.len(), 43);
}

#[test]
fn test_provider_registry() {
    let providers = acmers::providers::list();
    assert!(!providers.is_empty());

    let cf = acmers::providers::find("cf");
    assert!(cf.is_some());
    assert_eq!(cf.unwrap().slug, "cf");

    let unknown = acmers::providers::find("nonexistent");
    assert!(unknown.is_none());
}
