# Spec: DNS Provider Implementation Pattern

## Overview

All DNS providers in acmers follow a consistent pattern defined by the `DnsProvider` trait. This spec documents the standard implementation pattern used across all 201 providers.

## Trait Definition

```rust
pub trait DnsProvider: Send + Sync {
    fn slug() -> &'static str where Self: Sized;
    fn env_vars() -> &'static [&'static str] where Self: Sized;
    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult;
    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult;
}
```

## Standard Implementation Structure

### 1. Struct Definition

```rust
pub struct ProviderName {
    api_key: String,
    // other auth fields
}
```

### 2. Trait Implementation

```rust
impl DnsProvider for ProviderName {
    fn slug() -> &'static str { "provider_slug" }
    
    fn env_vars() -> &'static [&'static str] {
        &["PROVIDER_API_KEY", "PROVIDER_SECRET"]
    }
    
    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        let api_key = env.get("PROVIDER_API_KEY")
            .ok_or_else(|| Error::Config("PROVIDER_API_KEY required".into()))?
            .clone();
        Ok(Box::new(ProviderName { api_key }))
    }
    
    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        // 1. Resolve zone/domain
        // 2. Create TXT record via API
        // 3. Return Ok(()) or Err(...)
    }
    
    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        // 1. Resolve zone/domain
        // 2. List records to find ID
        // 3. Delete record via API
        // 4. Return Ok(()) (idempotent)
    }
}
```

## Common Authentication Patterns

### Bearer Token
```rust
let auth = format!("Bearer {}", token);
http::post(url, body, "application/json", &[("Authorization", &auth)])
```

### Basic Auth
```rust
let creds = base64::encode_std(format!("{user}:{pass}").as_bytes());
let auth = format!("Basic {creds}");
http::get(url, &[("Authorization", &auth)])
```

### API Key Header
```rust
http::get(url, &[("X-API-Key", api_key)])
```

### HMAC Signing
```rust
let signature = hmac_sha256(secret, message);
let auth = format!("HMAC-SHA256 Signature={}", base64::encode_std(&signature));
```

### OAuth2
```rust
// Get token
let token = get_oauth_token(client_id, client_secret);
// Use token
let auth = format!("Bearer {}", token);
```

## Zone Resolution Patterns

### Direct Zone ID
```rust
fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
    Ok(self.zone_id.clone())
}
```

### Iterative Suffix Matching
```rust
fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
    let mut search = domain.to_string();
    loop {
        // Try to find zone for `search`
        if let Some(zone) = find_zone(&search) {
            return Ok(zone);
        }
        // Strip leftmost label
        if let Some(pos) = search.find('.') {
            search = search[pos + 1..].to_string();
        } else {
            break;
        }
    }
    Err(Error::Provider(format!("zone not found for {domain}")))
}
```

### API Zone Listing
```rust
fn resolve_zone(&self, domain: &str) -> Result<String, Error> {
    let zones = http::get("/zones")?;
    for zone in zones {
        if domain == zone.name || domain.ends_with(&format!(".{}", zone.name)) {
            return Ok(zone.id);
        }
    }
    Err(Error::Provider(format!("zone not found for {domain}")))
}
```

## Record Operations

### Add TXT Record
```rust
fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
    let zone_id = self.resolve_zone(domain)?;
    let body = serde_json::json!({
        "type": "TXT",
        "name": name,
        "content": value,
        "ttl": 120
    });
    http::post(&format!("/zones/{}/records", zone_id), 
               &serde_json::to_vec(&body).unwrap(),
               "application/json", &auth_headers)?;
    Ok(())
}
```

### Remove TXT Record
```rust
fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
    let zone_id = self.resolve_zone(domain)?;
    let records = http::get(&format!("/zones/{}/records?type=TXT&name={}", 
                                     zone_id, name), &auth_headers)?;
    for record in records {
        if record.content == value {
            http::delete(&format!("/zones/{}/records/{}", zone_id, record.id), 
                        &auth_headers)?;
        }
    }
    Ok(())
}
```

## Error Handling

### Provider Errors
```rust
Err(Error::Provider(format!("API error: {}", response.body)))
```

### Config Errors
```rust
Err(Error::Config("PROVIDER_API_KEY required".into()))
```

### JSON Errors
```rust
Err(Error::Json(format!("parse error: {}", e)))
```

## Testing Pattern

```rust
#[test]
fn test_provider_add_txt() {
    let handler = Arc::new(|method, path, body, headers| {
        if method == "POST" && path.contains("/records") {
            return (200, r#"{"id":"123"}"#.to_string(), HashMap::new());
        }
        (404, "{}".to_string(), HashMap::new())
    });
    
    let server = MockServer::new(handler);
    http::set_test_base(&server.url());
    
    let mut env = HashMap::new();
    env.insert("PROVIDER_API_KEY".to_string(), "test-key".to_string());
    let provider = ProviderName::new(&env).unwrap();
    
    let result = provider.add_txt("example.com", "_acme-challenge", "value");
    assert!(result.is_ok());
}
```

## Checklist for New Providers

- [ ] Implement `DnsProvider` trait
- [ ] Define `slug()` matching acme.sh
- [ ] Define `env_vars()` matching acme.sh
- [ ] Implement `new()` with proper error handling
- [ ] Implement `add_txt()` with zone resolution
- [ ] Implement `remove_txt()` (idempotent)
- [ ] Add to `src/providers/mod.rs` registry
- [ ] Add construction test
- [ ] Add mock HTTP test
- [ ] Verify with `cargo build` (zero warnings)
- [ ] Verify with `cargo test` (all tests pass)

## Common Pitfalls

1. **Zone resolution**: Always implement proper zone resolution, don't assume domain == zone
2. **Record names**: Some APIs want FQDN, others want relative names
3. **TTL values**: Match acme.sh defaults (usually 60-300 seconds)
4. **Pagination**: Handle paginated API responses
5. **Multi-value TXT**: Preserve existing TXT values when adding new ones
6. **Idempotent remove**: `remove_txt` should not error if record doesn't exist
7. **Auth headers**: Match acme.sh exactly (Bearer vs Basic vs custom headers)
8. **Env var names**: Match acme.sh exactly (case-sensitive)
