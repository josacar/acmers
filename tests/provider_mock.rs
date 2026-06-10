mod mock;
use mock::MockServer;

use std::collections::HashMap;
use std::sync::Arc;

#[test]
fn test_cloudflare_add_txt() {
    let handler: Arc<dyn Fn(&str, &str, &[u8], &HashMap<String, String>) -> (u16, String, HashMap<String, String>) + Send + Sync> = Arc::new(|method, path, _body, _headers| {
        // Zone validation: GET /client/v4/zones/{zone_id}
        if method == "GET" && path == "/client/v4/zones/zone123" {
            return (200, serde_json::json!({
                "result": {
                    "id": "zone123",
                    "name": "example.com",
                    "status": "active"
                },
                "success": true
            }).to_string(), HashMap::new());
        }
        
        // Zone search: GET /client/v4/zones?name=...
        if method == "GET" && path.starts_with("/client/v4/zones?") {
            return (200, serde_json::json!({
                "result": [{
                    "id": "zone123",
                    "name": "example.com",
                    "status": "active"
                }],
                "success": true
            }).to_string(), HashMap::new());
        }

        // List DNS records: GET /client/v4/zones/{zone_id}/dns_records
        if method == "GET" && path.contains("/client/v4/zones/zone123/dns_records") {
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

        // Delete DNS record: DELETE /client/v4/zones/{zone_id}/dns_records/{id}
        if method == "DELETE" && path.starts_with("/client/v4/zones/zone123/dns_records/") {
            return (200, serde_json::json!({"success": true}).to_string(), HashMap::new());
        }

        // Add DNS record: POST /client/v4/zones/{zone_id}/dns_records
        if method == "POST" && path == "/client/v4/zones/zone123/dns_records" {
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
        // List domains: GET /v2/domains (exact match, must come first)
        if method == "GET" && path == "/v2/domains" {
            return (200, serde_json::json!({
                "domains": [{"name": "example.com", "ttl": 1800}],
                "meta": {"total": 1}
            }).to_string(), HashMap::new());
        }

        // Add DNS record: POST /v2/domains/{domain}/records
        if method == "POST" && path == "/v2/domains/example.com/records" {
            return (201, serde_json::json!({
                "domain_record": {
                    "id": 12345,
                    "type": "TXT",
                    "name": "_acme-challenge",
                    "data": "test-value"
                }
            }).to_string(), HashMap::new());
        }

        // List DNS records: GET /v2/domains/{domain}/records
        if method == "GET" && path == "/v2/domains/example.com/records" {
            return (200, serde_json::json!({
                "domain_records": [{
                    "id": 12345,
                    "type": "TXT",
                    "name": "_acme-challenge",
                    "data": "test-value"
                }]
            }).to_string(), HashMap::new());
        }

        // Delete DNS record: DELETE /v2/domains/{domain}/records/{id}
        if method == "DELETE" && path.starts_with("/v2/domains/") && path.contains("/records/") {
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
        // List records: GET /v1/domains/{domain}/records
        if method == "GET" && path == "/v1/domains/example.com/records" {
            return (200, "[]".to_string(), HashMap::new());
        }
        
        // Update records: PUT /v1/domains/{domain}/records
        if method == "PUT" && path == "/v1/domains/example.com/records" {
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

#[test]
fn test_kas_add_txt() {
    let handler: Arc<dyn Fn(&str, &str, &[u8], &HashMap<String, String>) -> (u16, String, HashMap<String, String>) + Send + Sync> = Arc::new(|method, path, body, _headers| {
        if method == "GET" && path.contains("KasApi.wsdl") {
            let wsdl = r#"<?xml version="1.0"?><definitions xmlns:soap="http://schemas.xmlsoap.org/wsdl/soap/"><service name="KasApiService"><port name="KasApiPort"><soap:address location="https://kasapi.kasserver.com/soap/v1/KasApi.php"/></port></service></definitions>"#;
            return (200, wsdl.to_string(), HashMap::new());
        }
        if method == "GET" && path.contains("KasAuth.wsdl") {
            let wsdl = r#"<?xml version="1.0"?><definitions xmlns:soap="http://schemas.xmlsoap.org/wsdl/soap/"><service name="KasAuthService"><port name="KasAuthPort"><soap:address location="https://kasapi.kasserver.com/soap/v1/KasAuth.php"/></port></service></definitions>"#;
            return (200, wsdl.to_string(), HashMap::new());
        }
        if method == "POST" && path.contains("KasAuth.php") {
            let resp = r#"<?xml version="1.0"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/"><SOAP-ENV:Body><ns1:KasAuthResponse xmlns:ns1="urn:xmethodsKasApiAuthentication"><return xsi:type="xsd:string">test-credential-token-12345</return></ns1:KasAuthResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>"#;
            return (200, resp.to_string(), HashMap::new());
        }
        if method == "POST" && path.contains("KasApi.php") {
            let body_str = String::from_utf8_lossy(body);
            if body_str.contains(r#""kas_action":"get_domains""#) {
                let resp = r#"<?xml version="1.0"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/"><SOAP-ENV:Body><ns1:KasApiResponse xmlns:ns1="urn:xmethodsKasApi"><return xsi:type="xsd:string"><item><key xsi:type="xsd:string">domain_name</key><value xsi:type="xsd:string">example.com</value></item></return></ns1:KasApiResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>"#;
                return (200, resp.to_string(), HashMap::new());
            }
            if body_str.contains(r#""kas_action":"get_dns_settings""#) {
                let resp = r#"<?xml version="1.0"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/"><SOAP-ENV:Body><ns1:KasApiResponse xmlns:ns1="urn:xmethodsKasApi"><return xsi:type="xsd:string"></return></ns1:KasApiResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>"#;
                return (200, resp.to_string(), HashMap::new());
            }
            if body_str.contains(r#""kas_action":"add_dns_settings""#) {
                let resp = r#"<?xml version="1.0"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/"><SOAP-ENV:Body><ns1:KasApiResponse xmlns:ns1="urn:xmethodsKasApi"><return xsi:type="xsd:string"><item><key xsi:type="xsd:string">ReturnString</key><value xsi:type="xsd:string">TRUE</value></item></return></ns1:KasApiResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>"#;
                return (200, resp.to_string(), HashMap::new());
            }
            if body_str.contains(r#""kas_action":"delete_dns_settings""#) {
                let resp = r#"<?xml version="1.0"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/"><SOAP-ENV:Body><ns1:KasApiResponse xmlns:ns1="urn:xmethodsKasApi"><return xsi:type="xsd:string"><item><key xsi:type="xsd:string">ReturnString</key><value xsi:type="xsd:string">TRUE</value></item></return></ns1:KasApiResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>"#;
                return (200, resp.to_string(), HashMap::new());
            }
        }
        (404, "{}".to_string(), HashMap::new())
    });

    let server = MockServer::new(handler);
    acmers::http::set_test_base(&server.url());

    let mut env = HashMap::new();
    env.insert("KAS_Login".to_string(), "test-login".to_string());
    env.insert("KAS_Authtype".to_string(), "plain".to_string());
    env.insert("KAS_Authdata".to_string(), "test-password".to_string());
    let kas = acmers::providers::find("kas").unwrap();
    let provider = (kas.create)(&env).unwrap();

    let result = provider.add_txt("example.com", "_acme-challenge.example.com", "test-challenge-value");
    assert!(result.is_ok(), "KAS add_txt failed: {:?}", result.err());
}

#[test]
fn test_kas_remove_txt() {
    let handler: Arc<dyn Fn(&str, &str, &[u8], &HashMap<String, String>) -> (u16, String, HashMap<String, String>) + Send + Sync> = Arc::new(|method, path, body, _headers| {
        if method == "GET" && path.contains("KasApi.wsdl") {
            let wsdl = r#"<?xml version="1.0"?><definitions xmlns:soap="http://schemas.xmlsoap.org/wsdl/soap/"><service name="KasApiService"><port name="KasApiPort"><soap:address location="https://kasapi.kasserver.com/soap/v1/KasApi.php"/></port></service></definitions>"#;
            return (200, wsdl.to_string(), HashMap::new());
        }
        if method == "GET" && path.contains("KasAuth.wsdl") {
            let wsdl = r#"<?xml version="1.0"?><definitions xmlns:soap="http://schemas.xmlsoap.org/wsdl/soap/"><service name="KasAuthService"><port name="KasAuthPort"><soap:address location="https://kasapi.kasserver.com/soap/v1/KasAuth.php"/></port></service></definitions>"#;
            return (200, wsdl.to_string(), HashMap::new());
        }
        if method == "POST" && path.contains("KasAuth.php") {
            let resp = r#"<?xml version="1.0"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/"><SOAP-ENV:Body><ns1:KasAuthResponse xmlns:ns1="urn:xmethodsKasApiAuthentication"><return xsi:type="xsd:string">test-credential-token-12345</return></ns1:KasAuthResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>"#;
            return (200, resp.to_string(), HashMap::new());
        }
        if method == "POST" && path.contains("KasApi.php") {
            let body_str = String::from_utf8_lossy(body);
            if body_str.contains(r#""kas_action":"get_domains""#) {
                let resp = r#"<?xml version="1.0"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/"><SOAP-ENV:Body><ns1:KasApiResponse xmlns:ns1="urn:xmethodsKasApi"><return xsi:type="xsd:string"><item><key xsi:type="xsd:string">domain_name</key><value xsi:type="xsd:string">example.com</value></item></return></ns1:KasApiResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>"#;
                return (200, resp.to_string(), HashMap::new());
            }
            if body_str.contains(r#""kas_action":"get_dns_settings""#) {
                let resp = r#"<?xml version="1.0"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/"><SOAP-ENV:Body><ns1:KasApiResponse xmlns:ns1="urn:xmethodsKasApi"><return xsi:type="xsd:string"><item xsi:type="ns2:Map"><item><key xsi:type="xsd:string">record_id</key><value xsi:type="xsd:string">99999</value></item><item><key xsi:type="xsd:string">record_name</key><value xsi:type="xsd:string">_acme-challenge</value></item><item><key xsi:type="xsd:string">record_type</key><value xsi:type="xsd:string">TXT</value></item><item><key xsi:type="xsd:string">record_data</key><value xsi:type="xsd:string">test-challenge-value</value></item></item></return></ns1:KasApiResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>"#;
                return (200, resp.to_string(), HashMap::new());
            }
            if body_str.contains(r#""kas_action":"delete_dns_settings""#) {
                let resp = r#"<?xml version="1.0"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/"><SOAP-ENV:Body><ns1:KasApiResponse xmlns:ns1="urn:xmethodsKasApi"><return xsi:type="xsd:string"><item><key xsi:type="xsd:string">ReturnString</key><value xsi:type="xsd:string">TRUE</value></item></return></ns1:KasApiResponse></SOAP-ENV:Body></SOAP-ENV:Envelope>"#;
                return (200, resp.to_string(), HashMap::new());
            }
        }
        (404, "{}".to_string(), HashMap::new())
    });

    let server = MockServer::new(handler);
    acmers::http::set_test_base(&server.url());

    let mut env = HashMap::new();
    env.insert("KAS_Login".to_string(), "test-login".to_string());
    env.insert("KAS_Authtype".to_string(), "plain".to_string());
    env.insert("KAS_Authdata".to_string(), "test-password".to_string());
    let kas = acmers::providers::find("kas").unwrap();
    let provider = (kas.create)(&env).unwrap();

    let result = provider.remove_txt("example.com", "_acme-challenge.example.com", "test-challenge-value");
    assert!(result.is_ok(), "KAS remove_txt failed: {:?}", result.err());
}
