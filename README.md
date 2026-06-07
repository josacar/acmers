# acmers

A minimal-dependency Rust CLI for automatic SSL/TLS certificate issuance and renewal
via the ACME protocol (RFC 8555), with DNS-01 challenge support for **200+ DNS providers**
ported from the [acme.sh](https://github.com/acmesh-official/acme.sh) project.

## Why

acme.sh is great but requires bash/curl/openssl. `acmers` does the same job with **8 pure-Rust
dependencies** — no system tools needed. Single binary, no runtime, no surprises.

## Quick Start

```sh
# Install from crates.io
cargo install acmers

# Or build from source
git clone https://github.com/josacar/acmers.git
cd acmers
cargo build --release
```

### Register an ACME account

```sh
# Production (Let's Encrypt)
acmers register --email you@example.com

# Test mode (staging)
acmers register --email you@example.com --test
```

### Issue a certificate

```sh
# Single domain, wildcard, or multi-domain
acmers issue -d example.com -d '*.example.com' --dns cf

# With provider-specific env vars
CF_Token=xxx acmers issue -d example.com --dns cf --email you@example.com

# Multi-domain across providers
acmers issue -d site1.com --dns cf -d site2.com --dns gd
```

### Renew a certificate

```sh
acmers renew -d example.com
```

### Automate with cron

```sh
# Add to crontab
0 0 * * * /usr/local/bin/acmers cron
```

### List available providers

```sh
acmers list-providers | head -20
# Output:
#   cf                   env: CF_Token, CF_Zone_ID, CF_Account_ID
#   dgon                 env: DO_API_KEY
#   aws                  env: AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY
#   ...
```

### Revoke a certificate

```sh
acmers revoke -d example.com
```

## Configuration

All configuration is via environment variables. Credentials are stored in `~/.acmers/`.

```
~/.acmers/
├── account.json       # ACME account key and URL
├── example.com/
│   ├── cert.pem       # Certificate
│   ├── key.pem        # Private key (PKCS#8 DER)
│   ├── fullchain.pem  # Full chain
│   └── renewal.json   # Renewal config (provider, email, server)
└── other-domain.com/
    └── ...
```

### Supported CAs

| CA | Server URL | Flag |
|----|-----------|------|
| Let's Encrypt | `https://acme-v02.api.letsencrypt.org/directory` | (default) |
| Let's Encrypt Staging | `https://acme-staging-v02.api.letsencrypt.org/directory` | `--test` |
| ZeroSSL | `https://acme.zerossl.com/v2/DV90` | `--server URL` |
| SSL.com | `https://acme.ssl.com/sslcom-dv-ecc` | `--server URL` |
| Google Public CA | `https://dv.acme-v02.api.pki.goog/directory` | `--server URL` |

## Dependencies

| Crate | Version | Debian Package | Purpose |
|-------|---------|---------------|---------|
| `ureq` | 3.3 | `librust-ureq-dev` | HTTP client (sync, pure Rust TLS) |
| `serde_json` | 1.0 | `librust-serde-json-dev` | JSON parsing |
| `rcgen` | 0.14 | `librust-rcgen-dev` | CSR creation |
| `ring` | 0.17 | `librust-ring-dev` | ECDSA keygen, JWS signing |
| `lexopt` | 0.3 | `librust-lexopt-dev` | CLI parsing |
| `time` | 0.3 | — | Certificate expiry parsing |
| `x509-parser` | 0.18 | — | X.509 certificate parsing |
| `rustls-pki-types` | 1 | — | Key DER types |

All pure Rust. No C/C++/OpenSSL dependencies. All available in Debian Sid.

Binary size: ~3.6 MB (release).

## Architecture

```
src/
├── main.rs           CLI dispatch, cron, renew, revoke
├── cli.rs            Argument parser
├── crypto.rs         ECDSA keygen, JWS ES256, CSR creation
├── http.rs           ureq wrapper (captures error bodies)
├── json.rs           serde_json field extraction helpers
├── base64.rs         Base64url encode/decode (pure Rust)
├── error.rs          Error types
├── config.rs         ~/.acmers/ directory management
├── acme/
│   ├── account.rs    Account registration and loading
│   ├── directory.rs  ACME directory discovery
│   ├── challenge.rs  Authorization, challenge, polling
│   └── order.rs      Order lifecycle, finalize, cert download
├── providers/
│   ├── mod.rs        DnsProvider trait + registry (201 providers)
│   ├── helpers.rs    Common REST patterns
│   ├── cf.rs         Cloudflare
│   ├── aws.rs        AWS Route53
│   ├── azure.rs      Azure DNS
│   ├── gcloud.rs     Google Cloud DNS
│   ├── ali.rs        Aliyun
│   ├── tencent.rs    TencentCloud
│   ├── ovh.rs        OVH
│   ├── dp.rs         DNSPod.cn
│   ├── gd.rs         GoDaddy
│   ├── dgon.rs       DigitalOcean
│   ├── gandi_livedns.rs  Gandi
│   ├── porkbun.rs    Porkbun
│   ├── namecheap.rs  Namecheap
│   ├── namecom.rs    Name.com
│   ├── dnsimple.rs   DNSimple
│   ├── vercel.rs     Vercel
│   ├── linode_v4.rs  Linode
│   ├── hetznercloud.rs  Hetzner
│   ├── duckdns.rs    DuckDNS
│   ├── ... and 180+ more
│   └── helpers.rs
├── tests/
│   └── integration/  Integration tests
└── docs/
    └── providers/    Provider API reference
```

## Provider Status

All 201 DNS providers from acme.sh are registered in the static registry.
Each provider implements the `DnsProvider` trait with `add_txt` and `remove_txt` methods.

| Status | Count | Description |
|--------|-------|-------------|
| **Implemented** | 68 | Full API calls (add/remove TXT records) |
| **Stub** | 133 | No-op stubs, ready for implementation |

### Implemented Providers (68)

**Cloud/DNS platforms:** AWS Route53, Google Cloud DNS, Azure DNS, Aliyun, TencentCloud,
OVH, DNSPod.cn, Yandex Cloud, Cloudflare, DigitalOcean, GoDaddy, Gandi, Porkbun,
Namecheap, Name.com, DNSimple, Vercel, Linode v4, Hetzner Cloud, IONOS

**Managed DNS:** ClouDNS, Bunny DNS, deSEC, Njalla, Netlify, Scaleway, Constellix,
Vultr, Exoscale, Dynv6, Rage4, G-Core, EdgeCenter, ACME-DNS

**Hosting/deployment:** DirectAdmin, Active24, Simply.com (UnoEuro), Mythic Beasts,
World4You, Variomedia, Domeneshop, RackCorp, Vscale, ConoHa, EUServ

**Simple DNS:** DuckDNS, FreeMyIP, HE DDNS, DreamHost, DNSExit

**Region-specific:** PointHQ (NZ), Misaka.io (JP), SiteHost (NZ), BookMyName (DE),
Websupport (SK), Infomaniak (CH), MyDevil (PL), Mijn.host (NL), Nodion (DE),
Internet.bs (BS), DurableDNS (US), OpenProvider REST (NL), Alwaysdata (FR), Restena (LU),
Timeweb Cloud (RU), EasyDNS (CA), Technitium (IN)

### Adding a Provider

Each provider is a 30-100 line Rust file. See the [AGENTS.md](AGENTS.md) guide for the
implementation pattern.

```rust
// src/providers/myprovider.rs
use std::collections::HashMap;
use crate::error::Error;
use crate::providers::{DnsProvider, ProviderResult};

pub struct MyProvider { api_key: String }

impl DnsProvider for MyProvider {
    fn slug() -> &'static str { "myprovider" }
    fn env_vars() -> &'static [&'static str] { &["MYPROVIDER_API_KEY"] }
    fn new(env: &HashMap<String, String>) -> Result<Box<dyn DnsProvider>, Error> { ... }
    fn add_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult { ... }
    fn remove_txt(&self, domain: &str, name: &str, value: &str) -> ProviderResult { ... }
}
```

Then register in `src/providers/mod.rs`:

```rust
use crate::providers::myprovider::MyProvider;

pub static PROVIDERS: &[ProviderMeta] = &[
    // ...
    ProviderMeta {
        slug: "myprovider",
        name: "MyProvider",
        env_vars: &["MYPROVIDER_API_KEY"],
        create: |env| MyProvider::new(env),
    },
];
```

## Testing

```sh
# Run all tests
cargo test

# Run integration tests against Let's Encrypt staging
cargo test --test integration -- --test-threads=1
```

## License

MIT © 2026
