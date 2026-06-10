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

#[test]
fn test_oci_add_txt() {
    let handler: Arc<dyn Fn(&str, &str, &[u8], &HashMap<String, String>) -> (u16, String, HashMap<String, String>) + Send + Sync> = Arc::new(|method, path, _body, headers| {
        if method == "GET" && path == "/20180115/zones/example.com" {
            return (200, serde_json::json!({
                "id": "ocid1.dns-zone.oc1..test",
                "name": "example.com",
                "zoneType": "PRIMARY"
            }).to_string(), HashMap::new());
        }
        if method == "PATCH" && path == "/20180115/zones/example.com/records" {
            assert!(headers.contains_key("authorization"), "missing Authorization header");
            let auth = headers.get("authorization").unwrap();
            assert!(auth.starts_with("Signature "), "bad auth scheme");
            assert!(auth.contains("keyId=\"ocid1.tenancy.oc1..test/ocid1.user.oc1..test/"), "bad keyId");
            return (200, "[]".to_string(), HashMap::new());
        }
        (404, "{}".to_string(), HashMap::new())
    });

    let server = MockServer::new(handler);
    acmers::http::set_test_base(&server.url());

    let test_key = "-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQDxRGYu/L00KteB\n1Ls7aT0zwSYdTS3k4OBWUkkN7FMfi8vVHCu4UCOX8dJQd08WRsGrnE+Dcu5c61rA\nYDImhIk5YjeIFNPY0HemhwStA/z1GpHKoCQzmkkn8NWmfhNoCozc1ja6MVm4eIL1\nzWuKia2+DkmXyW8rn1p+nSjrlu2D/BDe/wMjXYvP+ZBC2E4qMdJs41ue/mHWDe1E\n4jKRDjKmt+azceeCpnskpmAB49K2xuTznJB2t25iXfoEhNCO1jU0Y94fBHOgSdaG\nMuI6VsWjBKWcpLeUAOQO8YC8wgSbeIt9AiPruydR+YQPmy8w7H/SgdFQoFkH3qWV\n2rqWaVMZAgMBAAECggEAAslc5/9AIqKICfdLUvRwLn2mxOIggyb4+7KjJncU+ON/\nAZxfAaxiw5DsD0kmpU4G8Fr1qMoSGeCTj/tAD75HTDy6VG857A/DTFb9ZUFFJmD8\nusu2QWSEDetdPWKO0AxlFPGPqkFhysXffRbX3400PlpCBXgZ91jKPuEXa6zHU8Ks\nizk83yxc4wp7CmkX2VbRf4UAf2gQaPL5w83J5A45L87deRWK//IijQwotS4svR77\nZIQxCjlnFc9udkIP0JvihfIv8tcY1L/HdRscV7sximwKmKbH0mvXwgNYA5Corau8\noRsi/TB1vOSJYOqlnUhmx4xbmZGAmmFoO+dWTnnpvQKBgQD4mJkFU9wNJeKJ3Tzd\n6EKtHJDhaTuQMvqk9OXH3U7G5F+cQ+U5RQ1ZbJkMS5Z1T1wP28egtNZVnvujKFkD\n77nBYY4XpOLhGpXx83YxsRrEZSCuNPtgNJrGDFQwYoDCWtgNGYAdIw8cHeIXJ5Y9\n91pCm164Qb8vaDv4J/ry6h1IuwKBgQD4c+w6FyMVD+sYmUC7v9v3eeqAI0OmbfHd\nNWAHaNtEq1ywyZVQjlF+P9B3nyiSSEFHDXokWBbgO45d4cPpp6r3ioCa2aGwYUwi\nuNEUxlsHHww9TJj64w1yXAe1a224sAHUBNLvRZ+nKSd9vV0v6jfqxo4uwtBpmx2F\nokycdOuwOwKBgC5vwtW97nL+SqzaCM6i3iGcHmwcziWHgE5j+LA25Mo+SqXUAPOL\ntIypvoUPcZGEO3wy371jSk5AHl1B4i7cDuTSpkpAYKkP4EaL5d4uaQOaqFoiR3qX\nGPo5v1gybj7f3U/FHatTqzTjWCJfIK9+jvu2LiFZFq9yVxFp1nSdys6VAoGAJfHG\nXSTVdc0Fka8uJL5rgMM83i8EkPFvo+IX9Wm9OyKUuGdBB5mEtqxWUT6ceqLQXWKg\nidZuP/a4inwFaLTztnSPqZadTAvADfl97RdSJadHPkFph7+PeSy2/K0Yh8FRtii6\nclKGzIfLgTefeMbjnVaPtnKIU+idvKAJ5UcyC6sCgYEA6tQSW/2c1rOBuwdSYvyd\nbSm5dyf+srGvyIsAOS9shBVOwQcYa5UoFO5EgD6I066wvGtwhWJWGROLwZy6qvBb\n1cPYOj3pFrtkCpl5tgfI7YDztXZhKE3KOQCRSLrgQ/a6EM2vA8KwIqXLqqsHUPJJ\nZ5fZtc/KoTqLNc6P1I3/2fA=\n-----END PRIVATE KEY-----";

    let mut env = HashMap::new();
    env.insert("OCI_PRIVKEY".to_string(), test_key.to_string());
    env.insert("OCI_TENANCY".to_string(), "ocid1.tenancy.oc1..test".to_string());
    env.insert("OCI_USER".to_string(), "ocid1.user.oc1..test".to_string());
    env.insert("OCI_REGION".to_string(), "us-phoenix-1".to_string());
    let oci = acmers::providers::find("oci").unwrap();
    let provider = (oci.create)(&env).unwrap();

    let result = provider.add_txt("example.com", "_acme-challenge.example.com", "test-challenge-value");
    assert!(result.is_ok(), "OCI add_txt failed: {:?}", result.err());
}

#[test]
fn test_oci_remove_txt() {
    let handler: Arc<dyn Fn(&str, &str, &[u8], &HashMap<String, String>) -> (u16, String, HashMap<String, String>) + Send + Sync> = Arc::new(|method, path, _body, _headers| {
        if method == "GET" && path == "/20180115/zones/example.com" {
            return (200, serde_json::json!({
                "id": "ocid1.dns-zone.oc1..test",
                "name": "example.com"
            }).to_string(), HashMap::new());
        }
        if method == "PATCH" && path == "/20180115/zones/example.com/records" {
            return (200, "[]".to_string(), HashMap::new());
        }
        (404, "{}".to_string(), HashMap::new())
    });

    let server = MockServer::new(handler);
    acmers::http::set_test_base(&server.url());

    let test_key = "-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQDxRGYu/L00KteB\n1Ls7aT0zwSYdTS3k4OBWUkkN7FMfi8vVHCu4UCOX8dJQd08WRsGrnE+Dcu5c61rA\nYDImhIk5YjeIFNPY0HemhwStA/z1GpHKoCQzmkkn8NWmfhNoCozc1ja6MVm4eIL1\nzWuKia2+DkmXyW8rn1p+nSjrlu2D/BDe/wMjXYvP+ZBC2E4qMdJs41ue/mHWDe1E\n4jKRDjKmt+azceeCpnskpmAB49K2xuTznJB2t25iXfoEhNCO1jU0Y94fBHOgSdaG\nMuI6VsWjBKWcpLeUAOQO8YC8wgSbeIt9AiPruydR+YQPmy8w7H/SgdFQoFkH3qWV\n2rqWaVMZAgMBAAECggEAAslc5/9AIqKICfdLUvRwLn2mxOIggyb4+7KjJncU+ON/\nAZxfAaxiw5DsD0kmpU4G8Fr1qMoSGeCTj/tAD75HTDy6VG857A/DTFb9ZUFFJmD8\nusu2QWSEDetdPWKO0AxlFPGPqkFhysXffRbX3400PlpCBXgZ91jKPuEXa6zHU8Ks\nizk83yxc4wp7CmkX2VbRf4UAf2gQaPL5w83J5A45L87deRWK//IijQwotS4svR77\nZIQxCjlnFc9udkIP0JvihfIv8tcY1L/HdRscV7sximwKmKbH0mvXwgNYA5Corau8\noRsi/TB1vOSJYOqlnUhmx4xbmZGAmmFoO+dWTnnpvQKBgQD4mJkFU9wNJeKJ3Tzd\n6EKtHJDhaTuQMvqk9OXH3U7G5F+cQ+U5RQ1ZbJkMS5Z1T1wP28egtNZVnvujKFkD\n77nBYY4XpOLhGpXx83YxsRrEZSCuNPtgNJrGDFQwYoDCWtgNGYAdIw8cHeIXJ5Y9\n91pCm164Qb8vaDv4J/ry6h1IuwKBgQD4c+w6FyMVD+sYmUC7v9v3eeqAI0OmbfHd\nNWAHaNtEq1ywyZVQjlF+P9B3nyiSSEFHDXokWBbgO45d4cPpp6r3ioCa2aGwYUwi\nuNEUxlsHHww9TJj64w1yXAe1a224sAHUBNLvRZ+nKSd9vV0v6jfqxo4uwtBpmx2F\nokycdOuwOwKBgC5vwtW97nL+SqzaCM6i3iGcHmwcziWHgE5j+LA25Mo+SqXUAPOL\ntIypvoUPcZGEO3wy371jSk5AHl1B4i7cDuTSpkpAYKkP4EaL5d4uaQOaqFoiR3qX\nGPo5v1gybj7f3U/FHatTqzTjWCJfIK9+jvu2LiFZFq9yVxFp1nSdys6VAoGAJfHG\nXSTVdc0Fka8uJL5rgMM83i8EkPFvo+IX9Wm9OyKUuGdBB5mEtqxWUT6ceqLQXWKg\nidZuP/a4inwFaLTztnSPqZadTAvADfl97RdSJadHPkFph7+PeSy2/K0Yh8FRtii6\nclKGzIfLgTefeMbjnVaPtnKIU+idvKAJ5UcyC6sCgYEA6tQSW/2c1rOBuwdSYvyd\nbSm5dyf+srGvyIsAOS9shBVOwQcYa5UoFO5EgD6I066wvGtwhWJWGROLwZy6qvBb\n1cPYOj3pFrtkCpl5tgfI7YDztXZhKE3KOQCRSLrgQ/a6EM2vA8KwIqXLqqsHUPJJ\nZ5fZtc/KoTqLNc6P1I3/2fA=\n-----END PRIVATE KEY-----";

    let mut env = HashMap::new();
    env.insert("OCI_PRIVKEY".to_string(), test_key.to_string());
    env.insert("OCI_TENANCY".to_string(), "ocid1.tenancy.oc1..test".to_string());
    env.insert("OCI_USER".to_string(), "ocid1.user.oc1..test".to_string());
    env.insert("OCI_REGION".to_string(), "us-phoenix-1".to_string());
    let oci = acmers::providers::find("oci").unwrap();
    let provider = (oci.create)(&env).unwrap();

    let result = provider.remove_txt("example.com", "_acme-challenge.example.com", "test-challenge-value");
    assert!(result.is_ok(), "OCI remove_txt failed: {:?}", result.err());
}

#[test]
fn test_joker_add_txt() {
    let handler: Arc<dyn Fn(&str, &str, &[u8], &HashMap<String, String>) -> (u16, String, HashMap<String, String>) + Send + Sync> = Arc::new(|method, path, body, _headers| {
        if method == "POST" && path.contains("/nic/replace") {
            let body_str = String::from_utf8_lossy(body);
            if body_str.contains("label=jokerTXTUpdateTest") {
                return (200, "OK: 0|1|...".to_string(), HashMap::new());
            }
            if body_str.contains("type=TXT") && body_str.contains("value=test-challenge-value") {
                return (200, "OK: 0|1|...".to_string(), HashMap::new());
            }
        }
        (404, "not found".to_string(), HashMap::new())
    });

    let server = MockServer::new(handler);
    acmers::http::set_test_base(&server.url());

    let mut env = HashMap::new();
    env.insert("JOKER_USERNAME".to_string(), "test-user".to_string());
    env.insert("JOKER_PASSWORD".to_string(), "test-pass".to_string());
    let joker = acmers::providers::find("joker").unwrap();
    let provider = (joker.create)(&env).unwrap();

    let result = provider.add_txt("example.com", "_acme-challenge.example.com", "test-challenge-value");
    assert!(result.is_ok(), "Joker add_txt failed: {:?}", result.err());
}

#[test]
fn test_joker_remove_txt() {
    let handler: Arc<dyn Fn(&str, &str, &[u8], &HashMap<String, String>) -> (u16, String, HashMap<String, String>) + Send + Sync> = Arc::new(|method, path, _body, _headers| {
        if method == "POST" && path.contains("/nic/replace") {
            return (200, "OK: 0|1|...".to_string(), HashMap::new());
        }
        (404, "not found".to_string(), HashMap::new())
    });

    let server = MockServer::new(handler);
    acmers::http::set_test_base(&server.url());

    let mut env = HashMap::new();
    env.insert("JOKER_USERNAME".to_string(), "test-user".to_string());
    env.insert("JOKER_PASSWORD".to_string(), "test-pass".to_string());
    let joker = acmers::providers::find("joker").unwrap();
    let provider = (joker.create)(&env).unwrap();

    let result = provider.remove_txt("example.com", "_acme-challenge.example.com", "test-challenge-value");
    assert!(result.is_ok(), "Joker remove_txt failed: {:?}", result.err());
}
