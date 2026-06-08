mod mock;
use mock::MockServer;

use std::collections::HashMap;
use std::sync::Arc;

#[test]
fn test_cloudflare_add_txt() {
    let handler: Arc<dyn Fn(&str, &str, &[u8], &HashMap<String, String>) -> (u16, String, HashMap<String, String>) + Send + Sync> = Arc::new(|method, path, _body, _headers| {
        if method == "GET" && path.contains("/zones/") {
            return (200, serde_json::json!({
                "result": {
                    "id": "zone123",
                    "name": "example.com",
                    "status": "active"
                },
                "success": true
            }).to_string(), HashMap::new());
        }
        
        if method == "GET" && path.contains("/zones?") {
            return (200, serde_json::json!({
                "result": [{
                    "id": "zone123",
                    "name": "example.com",
                    "status": "active"
                }],
                "success": true
            }).to_string(), HashMap::new());
        }
        
        if method == "POST" && path.contains("/dns_records") {
            return (200, serde_json::json!({
                "result": {
                    "id": "rec456",
                    "type": "TXT",
                    "name": "_acme-challenge.example.com",
                    "content": "\"test-challenge-value\"",
                    "ttl": 120
                },
                "success": true
            }).to_string(), HashMap::new());
        }

        if method == "GET" && path.contains("/dns_records") {
            return (200, serde_json::json!({
                "result": [{
                    "id": "rec456",
                    "type": "TXT",
                    "name": "_acme-challenge.example.com",
                    "content": "\"test-challenge-value\""
                }],
                "success": true
            }).to_string(), HashMap::new());
        }

        if method == "DELETE" && path.contains("/dns_records/") {
            return (200, serde_json::json!({"success": true}).to_string(), HashMap::new());
        }
        
        (404, r#"{"success":false,"errors":[{"message":"not found"}]}"#.to_string(), HashMap::new())
    });
    
    let server = MockServer::new(handler);
    acmers::http::set_test_base(&server.url());
    
    let mut env = HashMap::new();
    env.insert("CF_Token".to_string(), "test-token".to_string());
    env.insert("CF_Zone_ID".to_string(), "zone123".to_string());
    let cf = acmers::providers::find("cf").unwrap();
    let provider = (cf.create)(&env).unwrap();
    
    let result = provider.add_txt("example.com", "_acme-challenge.example.com", "test-challenge-value");
    assert!(result.is_ok(), "CF add_txt failed: {:?}", result.err());
}

#[test]
fn test_digitalocean_add_txt() {
    let handler: Arc<dyn Fn(&str, &str, &[u8], &HashMap<String, String>) -> (u16, String, HashMap<String, String>) + Send + Sync> = Arc::new(|method, path, _body, _headers| {
        if method == "GET" && path.contains("/domains") && !path.contains("/records") {
            return (200, serde_json::json!({
                "domains": [{"name": "example.com", "ttl": 1800}],
                "meta": {"total": 1}
            }).to_string(), HashMap::new());
        }
        
        if method == "POST" && path.contains("/records") {
            return (201, serde_json::json!({
                "domain_record": {
                    "id": 12345,
                    "type": "TXT",
                    "name": "_acme-challenge",
                    "data": "test-value"
                }
            }).to_string(), HashMap::new());
        }

        if method == "GET" && path.contains("/records") {
            return (200, serde_json::json!({
                "domain_records": [{
                    "id": 12345,
                    "type": "TXT",
                    "name": "_acme-challenge",
                    "data": "test-value"
                }]
            }).to_string(), HashMap::new());
        }

        if method == "DELETE" && path.contains("/records/") {
            return (204, "".to_string(), HashMap::new());
        }
        
        (404, "{}".to_string(), HashMap::new())
    });
    
    let server = MockServer::new(handler);
    acmers::http::set_test_base(&server.url());
    
    let mut env = HashMap::new();
    env.insert("DO_API_KEY".to_string(), "test-key".to_string());
    let dgon = acmers::providers::find("dgon").unwrap();
    let provider = (dgon.create)(&env).unwrap();
    
    let result = provider.add_txt("example.com", "_acme-challenge.example.com", "test-value");
    assert!(result.is_ok(), "DO add_txt failed: {:?}", result.err());
}

#[test]
fn test_duckdns_add_txt() {
    let handler: Arc<dyn Fn(&str, &str, &[u8], &HashMap<String, String>) -> (u16, String, HashMap<String, String>) + Send + Sync> = Arc::new(|_method, path, _body, _headers| {
        if path.contains("/update") {
            return (200, "OK".to_string(), HashMap::new());
        }
        (404, "{}".to_string(), HashMap::new())
    });
    
    let server = MockServer::new(handler);
    acmers::http::set_test_base(&server.url());
    
    let mut env = HashMap::new();
    env.insert("DuckDNS_Token".to_string(), "test-token".to_string());
    let duck = acmers::providers::find("duckdns").unwrap();
    let provider = (duck.create)(&env).unwrap();
    
    let result = provider.add_txt("example", "_acme-challenge", "test-value");
    assert!(result.is_ok(), "DuckDNS add_txt failed: {:?}", result.err());
}

#[test]
fn test_godaddy_add_txt() {
    let handler: Arc<dyn Fn(&str, &str, &[u8], &HashMap<String, String>) -> (u16, String, HashMap<String, String>) + Send + Sync> = Arc::new(|method, path, _body, _headers| {
        if method == "GET" && path.contains("/records") {
            return (200, "[]".to_string(), HashMap::new());
        }
        
        if method == "PUT" && path.contains("/records") {
            return (200, "{}".to_string(), HashMap::new());
        }
        
        (404, "{}".to_string(), HashMap::new())
    });
    
    let server = MockServer::new(handler);
    acmers::http::set_test_base(&server.url());
    
    let mut env = HashMap::new();
    env.insert("GD_Key".to_string(), "test-key".to_string());
    env.insert("GD_Secret".to_string(), "test-secret".to_string());
    let gd = acmers::providers::find("gd").unwrap();
    let provider = (gd.create)(&env).unwrap();
    
    let result = provider.add_txt("example.com", "_acme-challenge", "test-value");
    assert!(result.is_ok(), "GoDaddy add_txt failed: {:?}", result.err());
}

#[test]
fn test_porkbun_add_txt() {
    let handler: Arc<dyn Fn(&str, &str, &[u8], &HashMap<String, String>) -> (u16, String, HashMap<String, String>) + Send + Sync> = Arc::new(|_method, path, body, _headers| {
        if path.contains("/dns/create/") {
            return (200, serde_json::json!({"status": "SUCCESS", "id": 12345}).to_string(), HashMap::new());
        }
        if path.contains("/dns/retrieve/") {
            return (200, serde_json::json!({
                "status": "SUCCESS",
                "records": [{"id": "12345", "type": "TXT", "name": "_acme-challenge", "content": "test-value"}]
            }).to_string(), HashMap::new());
        }
        (200, "{}".to_string(), HashMap::new())
    });
    
    let server = MockServer::new(handler);
    acmers::http::set_test_base(&server.url());
    
    let mut env = HashMap::new();
    env.insert("PORKBUN_API_KEY".to_string(), "test-key".to_string());
    env.insert("PORKBUN_SECRET_API_KEY".to_string(), "test-secret".to_string());
    let pb = acmers::providers::find("porkbun").unwrap();
    let provider = (pb.create)(&env).unwrap();
    
    let result = provider.add_txt("example.com", "_acme-challenge", "test-value");
    assert!(result.is_ok(), "Porkbun add_txt failed: {:?}", result.err());
}
