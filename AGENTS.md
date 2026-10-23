# AGENTS.md — acmers Coding Guide

This file provides instructions for AI agents working on the `acmers` codebase.

## Project Constraints

- **No new dependencies** unless absolutely necessary and available as `librust-*-dev` in Debian
- **No async runtime** — everything is synchronous (ureq is blocking)
- **No system tool calls** (no `curl`, `openssl`, `dig`) — everything is pure Rust. The only intentional subprocess execution is user-configured deployment/reload hooks (`--reloadcmd`, `--deploy-hook`).
- **All providers must implement `DnsProvider`** and be registered in `src/providers/mod.rs`

## Development Tools

This project uses [mise](https://mise.jdx.dev/) for task management. Common operations:

```sh
mise run build          # cargo build
mise run test           # cargo test
mise run lint           # cargo clippy --all-targets --all-features -- -D warnings
mise run format         # cargo fmt --all
mise run check          # format-check + lint + test (runs all three)
mise run providers      # Count implemented vs stub providers
mise run release 0.3.0  # Bump version, tag, and push (triggers GitHub Actions)
```

Run `mise tasks` to see all available tasks. All tasks are defined in `mise.toml`.

## Mandatory Workflow for All Changes

**Every code change MUST follow this workflow using openspec skills:**

1. **Propose the change** — Use the `openspec-propose` skill to create a new change with proposal, design, specs, and tasks artifacts
2. **Implement the change** — Work through the tasks defined in the change
3. **Sync specs** — Use the `openspec-sync-specs` skill to update main specs from the change
4. **Archive when done** — Use the `openspec-archive-change` skill to archive the completed change

This workflow ensures all changes are documented, designed, and tracked in `openspec/changes/`. Do not implement changes without first proposing them through openspec.

## Provider Implementation Pattern

Every DNS provider is a Rust file in `src/providers/` that implements the `DnsProvider` trait.

### Minimal required structure

```rust
use std::collections::HashMap;
use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct ProviderName {
    // Auth fields (api_key, token, username, password, etc.)
    api_key: String,
}

impl DnsProvider for ProviderName {
    fn slug() -> &'static str { "shortname" }
    fn env_vars() -> &'static [&'static str] { &["PROVIDER_ENV_VAR"] }
    
    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> {
        Ok(Box::new(ProviderName {
            api_key: env.get("PROVIDER_ENV_VAR")
                .ok_or_else(|| Error::Config("PROVIDER_ENV_VAR required".into()))?
                .clone(),
        }))
    }
    
    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        // 1. Find zone/domain ID
        // 2. Create TXT record via API
        // 3. Return Ok(()) or Err(...)
        Ok(())
    }
    
    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
        // 1. List records
        // 2. Find matching record ID
        // 3. Delete record via API
        // 4. Return Ok(()) (idempotent — no error if record not found)
        Ok(())
    }
}
```

### Registry entry

After creating the provider, add it to `src/providers/mod.rs`:

```rust
pub mod myprovider;  // add module declaration

pub static PROVIDERS: &[ProviderMeta] = &[
    // ... existing entries ...
    ProviderMeta {
        slug: "myprovider",           // CLI identifier (--dns myprovider)
        name: "ProviderName",         // Display name
        env_vars: &["PROVIDER_ENV_VAR"],  // Required environment variables
        create: |env| myprovider::ProviderName::new(env),
    },
];
```

### HTTP patterns

Use these functions from `crate::http`:

```rust
// GET request
let resp = http::get(url, &[("Authorization", &format!("Bearer {}", token))])?;

// POST JSON
let body = serde_json::json!({"key": "value"});
let resp = http::post(url, &serde_json::to_vec(&body).unwrap(), "application/json", &[
    ("Authorization", "Bearer xxx"),
])?;

// POST form data
let form_data = format!("param1=value1&param2=value2");
let resp = http::post(url, form_data.as_bytes(), "application/x-www-form-urlencoded", &[])?;

// DELETE request
let resp = http::CLIENT.delete(url, &[])?;

// Response handling
if resp.status >= 400 {
    return Err(Error::Provider(format!("API error: {}", resp.body)));
}
```

### JSON patterns

```rust
use serde_json::Value;
use crate::json as j;

let v: Value = serde_json::from_str(&resp.body)?;

// Extract fields
let name = j::get_string_required(&v, &["data", "name"])?;
let arr = j::get_array(&v, &["data", "records"]);

// Check success
if v.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
    // ...
}
```

### Shared helpers

Use `crate::providers::helpers` for common provider operations:

```rust
use crate::providers::helpers;

// URL encoding
let encoded = helpers::url_encode(value);        // space -> %20
let form_encoded = helpers::url_encode_form(value); // space -> +

// TXT record content quoting
let quoted = helpers::txt_value(value);

// Common REST record CRUD
let zone_id = helpers::find_zone_by_domain(base_url, domain, &headers)?;
let record = helpers::find_record(list_url, &zone_id, "TXT", name, &headers)?;
let id = helpers::create_record(create_url, &zone_id, "TXT", name, &quoted, 120, &headers)?;
helpers::delete_record(delete_url, &zone_id, &id, &headers)?;
```

### Error types

```rust
// Config errors (missing env vars, bad config)
Error::Config("message".into())

// Provider errors (API failures, zone not found)
Error::Provider("message".into())

// JSON parsing errors
Error::Json("message".into())

// ACME protocol errors
Error::Acme { status: 400, detail: "body".into(), error_type: "badRequest".into() }
```

### Remove TXT record — idempotent

The `remove_txt` method should be idempotent — if the record doesn't exist, return `Ok(())`:

```rust
fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult {
    match self.find_record(domain, name, value) {
        Ok(Some(id)) => self.delete_record(domain, &id),
        Ok(None) => Ok(()),  // already deleted
        Err(e) => {
            eprintln!("warning: cleanup failed: {e}");
            Ok(())  // don't fail on cleanup
        }
    }
}
```

## Common Auth Patterns

### Bearer token
```rust
let auth_header = format!("Bearer {}", token);
http::get(url, &[("Authorization", &auth_header)])
```

### Basic auth
```rust
use crate::base64;
let creds = base64::encode_std(format!("{user}:{pass}").as_bytes());
let auth_header = format!("Basic {creds}");
```

### API key in query params
```rust
let url = format!("https://api.example.com/resource?apikey={key}&domain={domain}");
http::get(&url, &[])
```

### API key in custom header
```rust
let url = format!("https://api.example.com/resource");
http::get(url, &[("X-API-Key", key)])
```

### Token via login endpoint
```rust
let body = serde_json::json!({"username": user, "password": pass});
let resp = http::post(login_url, &serde_json::to_vec(&body).unwrap(), "application/json", &[])?;
let v: Value = serde_json::from_str(&resp.body)?;
let token = j::get_string_required(&v, &["token"])?.to_string();
```

## Code Style

- **No comments** unless the code is genuinely non-obvious
- 4-space indentation
- Use `String` for owned strings, `&str` for borrowed
- Use `map_err(|e| Error::Provider(format!(...)))` for error conversion
- Use `?` for error propagation
- Match provider env var naming to acme.sh conventions (e.g., `CF_Token`, `AWS_ACCESS_KEY_ID`)
- All providers must be `Send + Sync`
- Build with `cargo build` — must compile with zero errors

## Testing

```sh
# Build
cargo build

# Run all tests (109 tests as of 0.3.0)
cargo test

# Focused integration tests
cargo test --test certbot_parity
cargo test --test acme_mock

# Format, lint, and test
mise run check

# Check for unused code
cargo build 2>&1 | grep warning
```

## Provider Landscape

There are **201 provider files** in `src/providers/`, all registered in `src/providers/mod.rs`.

| Status | Count | Description |
|--------|-------|-------------|
| **Implemented** | 196 | Full API calls via `http::` functions or native protocols |
| **Stub** | 5 | Require external CLI tools or have no API |

### Stub providers (5)

Stubs are ~28-line files with no HTTP calls. They return `Err(Error::Provider(...))` from `add_txt`
with a message directing users to an alternative. The remaining stubs all require external CLI tools
or have no DNS management API:

| Slug | Reason | Alternative |
|------|--------|-------------|
| `df` | DynDNSFree.de has no TXT API | — |
| `hetzner` | Robot doesn't manage DNS zones | `--dns hetznercloud` |
| `knot` | Requires knsupdate CLI | `--dns nsupdate` |
| `lexicon` | Requires lexicon Python CLI | Use lexicon CLI directly |
| `nsd` | Requires nsupdate CLI | `--dns nsupdate` |

### Non-HTTP providers (real implementations)

Two providers don't use `http::` but are fully implemented:
- `nsupdate` (738 lines) — DNS UPDATE protocol via UDP/TCP sockets
- `maradns` (324 lines) — MaraDNS zone file manipulation

### How to identify a stub

A stub provider file:
1. Is exactly ~28 lines
2. Contains zero `http::` calls
3. Returns `Err(Error::Provider(...))` from `add_txt` with guidance text
4. `remove_txt` returns `Ok(())` (no-op)

## File Structure Reference

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI dispatch, issue/renew/revoke, lifecycle commands |
| `src/cli.rs` | Argument parser |
| `src/crypto.rs` | Key generation, JWS signing, CSR, PEM encoding |
| `src/http.rs` | HTTP client (ureq wrapper) |
| `src/json.rs` | JSON field extraction helpers |
| `src/base64.rs` | Base64url encode/decode |
| `src/error.rs` | Error type definitions |
| `src/config.rs` | Filesystem paths, atomic writes, permissions |
| `src/renewal.rs` | Renewal configuration and SAN preservation |
| `src/deploy.rs` | Install paths, reload/deploy hooks |
| `src/dns.rs` | UDP DNS TXT propagation polling |
| `src/envfile.rs` | `KEY=VALUE` credential file loading |
| `src/lock.rs` | Exclusive renewal lock with stale-PID recovery |
| `src/storage.rs` | Certificate file set output |
| `src/acme/account.rs` | Per-server account registration and loading |
| `src/acme/ari.rs` | ARI (RFC 9773) renewal info |
| `src/acme/directory.rs` | ACME directory discovery |
| `src/acme/challenge.rs` | Authorization, challenge, polling |
| `src/acme/order.rs` | Order lifecycle, finalize, chain download |
| `src/providers/mod.rs` | Provider trait + registry |
| `src/providers/helpers.rs` | Common REST helpers |
| `src/providers/*.rs` | Individual provider implementations |
| `debian/acmers.service` | Packaged renewal service |
| `debian/acmers.timer` | Twice-daily renewal timer |
| `debian/acmers.cron.d` | Non-systemd cron fallback |

### Runtime layout

| Path | Purpose |
|------|---------|
| `~/.acmers/` | Per-user state (default for non-root runs) |
| `/var/lib/acmers/` | System state used by root and the packaged service |
| `/etc/acmers/env` | Provider credentials for unattended renewal (mode 0600) |
| `/var/lib/acmers/accounts/` | ACME accounts keyed by server |
| `/var/lib/acmers/<name>/` | Certificate files and `renewal.json` |
