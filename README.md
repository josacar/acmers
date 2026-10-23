# acmers

A minimal-dependency Rust CLI for automatic SSL/TLS certificate issuance and renewal
via the ACME protocol (RFC 8555), with DNS-01 challenge support for **200+ DNS providers**
ported from the [acme.sh](https://github.com/acmesh-official/acme.sh) project.

## Why

acme.sh is great but requires bash/curl/openssl. `acmers` does the same job with **8 pure-Rust
dependencies** — no system tools needed. Single binary, no runtime, no surprises.

## Quick Start

```sh
# Debian/Ubuntu (.deb package)
# Download from GitHub Releases: https://github.com/josacar/acmers/releases
sudo dpkg -i acmers_0.3.0_amd64.deb

# Or from crates.io
cargo install acmers

# Or build from source
git clone https://github.com/josacar/acmers.git
cd acmers
cargo build --release
```

Debian packages require glibc >= 2.35 (Debian 12+, Ubuntu 22.04+). amd64 and arm64
architectures are available.

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
acmers issue -d example.com -d '*.example.com' --dns cf --email you@example.com

# With provider-specific env vars
CF_Token=xxx acmers issue -d example.com --dns cf --email you@example.com

# Install the certificate and reload nginx after renewal
acmers issue -d example.com --dns cf --email you@example.com \
  --fullchain-path /etc/nginx/ssl/example.com/fullchain.pem \
  --key-path /etc/nginx/ssl/example.com/privkey.pem \
  --reloadcmd 'systemctl reload nginx'
```

Issuance options are stored per certificate, so later renewals reuse the install
paths and reload command. DNS propagation is checked by polling resolvers; use
`--dns-servers` and `--dns-timeout` to tune it, or `--dnssleep` as a fallback.

### Renew a certificate

```sh
# One certificate
acmers renew -d example.com

# Every stored certificate (same as `acmers renew --all`)
acmers renew
```

Renewal preserves all SANs and wildcards. Use `--force` to renew regardless of
expiry, `--dry-run` to test against staging without deploying, and `--days` to
change the 30-day renewal window.

### Deploying certificates

`--cert-path`, `--key-path`, `--fullchain-path`, and `--chain-path` copy
artifacts to the paths your services read. `--reloadcmd` runs after a successful
renewal; `--deploy-hook` runs after that. Hooks receive `RENEWED_LINEAGE` and
`RENEWED_DOMAINS`. `--pre-hook` and `--post-hook` run once around a renewal run.
Deployment and hooks are skipped for skipped certificates, dry runs, and
staging certificates.

```sh
acmers renew --all \
  --pre-hook 'systemctl stop nginx' \
  --post-hook 'systemctl start nginx'
```

### Automatic renewal

The Debian package ships a systemd timer and service modeled on certbot:

- `acmers.timer` runs at 00:00 and 12:00 with up to 12 hours of random delay and
  `Persistent=true`, so a missed run catches up after boot.
- `acmers.service` runs `/usr/bin/acmers renew --all` as root, with
  `PrivateTmp=true` and `StateDirectory=acmers`.
- The timer is enabled automatically on install. The service is static and is
  started by the timer.
- On hosts without systemd, `/etc/cron.d/acmers` provides the same twice-daily
  fallback.

Credentials for unattended DNS renewal are read from `/etc/acmers/env`:

```sh
sudo install -d -m 0700 /etc/acmers
sudo install -m 0600 /dev/null /etc/acmers/env
sudoedit /etc/acmers/env
```

```sh
# /etc/acmers/env
CF_Token=your-cloudflare-api-token
```

`acmers.service` loads this file automatically. For cron or manual runs use
`--env-file`:

```sh
acmers renew --all --env-file /etc/acmers/env
```

Values from `-e KEY=VALUE` override the file, which overrides the process
environment.

For a user-level setup without the package, add:

```sh
0 */12 * * * /usr/local/bin/acmers renew --all --env-file ~/.acmers/env
```

### Moving an existing user store to the system timer

The packaged service runs as root and uses `/var/lib/acmers`. If certificates
were issued as a normal user under `~/.acmers`, migrate them once:

```sh
sudo install -d -m 0750 -o root -g root /var/lib/acmers
sudo cp -a ~/.acmers/. /var/lib/acmers/
sudo chown -R root:root /var/lib/acmers
sudo chmod -R go-rwx /var/lib/acmers
sudo systemctl start acmers.service
sudo acmers certificates
```

The first run imports the legacy `account.json`, treats expired certificates as
due, and rewrites the old DER `key.pem` as PEM.

### Manage certificates

```sh
acmers certificates          # list names, domains, and expiry
acmers show -d example.com   # show stored configuration
acmers show-account          # show the active ACME account
acmers delete -d example.com # remove local certificate data
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
acmers revoke -d example.com --reason keycompromise
acmers revoke -d example.com --delete-after-revoke
```

## Configuration

All provider configuration is via environment variables. Certificate state is
stored in `~/.acmers/` for user runs and in `/var/lib/acmers/` when running as
root, including the packaged systemd service. Provider credentials for
unattended renewal live in `/etc/acmers/env`.

```
~/.acmers/  (or /var/lib/acmers/)
├── accounts/
│   ├── acme-v02.api.letsencrypt.org_directory.json
│   └── acme-staging-v02.api.letsencrypt.org_directory.json
├── example.com/
│   ├── cert.pem       # Leaf certificate
│   ├── chain.pem      # Intermediate certificates
│   ├── fullchain.pem  # Leaf + intermediates
│   ├── privkey.pem    # Private key (PKCS#8 PEM, mode 0600)
│   ├── key.pem        # Same PEM key for backward compatibility
│   └── renewal.json   # Domains, provider, server, install paths, hooks
└── other-domain.com/
    └── ...
```

**Breaking change:** `key.pem` is now PEM instead of raw DER, and `cert.pem`
contains only the leaf certificate. Use `privkey.pem` for the key and `chain.pem`
for the intermediates.

### Supported CAs

| CA | Server URL | Flag |
|----|-----------|------|
| Let's Encrypt | `https://acme-v02.api.letsencrypt.org/directory` | (default) |
| Let's Encrypt Staging | `https://acme-staging-v02.api.letsencrypt.org/directory` | `--test` |

`--server URL` accepts another ACME directory. CAs that require External Account
Binding (ZeroSSL, Google Public CA, SSL.com) are not supported yet.

## Migrating from certbot

`acmers` produces the file set certbot users expect and can deploy it the same
way. See [docs/MIGRATION.md](docs/MIGRATION.md) for a full Cloudflare DNS-01
walkthrough, including Caddy and generic service deployment.

| certbot | acmers |
|---|---|
| `certbot certonly --dns-... -d example.com` | `acmers issue -d example.com --dns <provider> --email you@example.com` |
| `certbot renew` | `acmers renew --all` |
| `certbot renew --dry-run` | `acmers renew --all --dry-run` |
| `certbot renew --force-renewal` | `acmers renew --all --force` |
| `certbot revoke --cert-path ...` | `acmers revoke -d example.com --reason keycompromise` |
| `--cert-path`, `--key-path`, `--fullchain-path` | same flags |
| `--reloadcmd` | same flag |
| `--deploy-hook` | same flag |
| `--pre-hook`, `--post-hook` | same flags |
| `--preferred-chain` | same flag |

The short version:

1. Reuse the Cloudflare token from certbot's `dns-cloudflare.ini` as
   `CF_Token` in `/etc/acmers/env`.
2. Reissue with the apex first and the wildcard second, e.g.
   `-d example.com -d '*.example.com'`, plus the same install paths and reload
   command certbot used.
3. Remove the certbot renewal entry and let `acmers renew --all` run from the
   shipped systemd timer.

The DNS provider name is the acme.sh slug shown by `acmers list-providers`
(for example `cf` for Cloudflare), not certbot's plugin name.

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
├── main.rs           CLI dispatch, issue, renew, revoke, cron
├── cli.rs            Argument parser
├── crypto.rs         ECDSA keygen, JWS ES256, CSR creation, PEM encoding
├── http.rs           ureq wrapper (captures error bodies)
├── json.rs           serde_json field extraction helpers
├── base64.rs         Base64url encode/decode (pure Rust)
├── error.rs          Error types
├── config.rs         ~/.acmers/ paths, atomic writes, permissions
├── renewal.rs        Renewal configuration and SAN preservation
├── deploy.rs         Certificate installation and reload hooks
├── dns.rs            Minimal UDP DNS TXT propagation polling
├── acme/
│   ├── account.rs    Per-server account registration and loading
│   ├── ari.rs        ARI (RFC 9773) renewal info
│   ├── directory.rs  ACME directory discovery
│   ├── challenge.rs  Authorization, challenge, polling
│   └── order.rs      Order lifecycle, finalize, chain download
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
| **Implemented** | 196 | Full API calls (add/remove TXT records) |
| **Stub** | 5 | Require external CLI or have no API |

### Implemented Providers (196)

**Cloud/DNS platforms:** AWS Route53, Google Cloud DNS, Azure DNS, Aliyun, TencentCloud,
OVH, DNSPod.cn, Yandex Cloud, Cloudflare, DigitalOcean, GoDaddy, Gandi, Porkbun,
Namecheap, Name.com, DNSimple, Vercel, Linode (v3 & v4), Hetzner Cloud, IONOS, IONOS Cloud,
Oracle Cloud (OCI), Huawei Cloud, Baidu Cloud, JD Cloud, Scaleway, Online.net,
Google Domains, Yandex PDD, Yandex 360

**Managed DNS:** ClouDNS, Bunny DNS, deSEC, Njalla, Netlify, Constellix,
Vultr, Exoscale, Dynv6, Rage4, G-Core, EdgeCenter, ACME-DNS, NS1, NSD/NSUpdate,
PowerDNS, Infoblox, EfficientIP, MaraDNS, Technitium, LuaDNS, Zonomi, Zilore,
Curanet (scannet.dk/wannafind.dk/dandomain.dk)

**Hosting/deployment:** DirectAdmin, Active24, Simply.com (UnoEuro), Mythic Beasts,
World4You, Variomedia, Domeneshop, RackCorp, Vscale, ConoHa, EUServ, CPanel,
HestiaCP, Plesk, ISPConfig, DreamHost, Cyon, Fornex, Hosttech, Infomaniak,
One.com, OpenProvider (XML & REST)

**Simple DNS:** DuckDNS, FreeMyIP, HE DDNS, DNSExit, Dynu, FreeDNS, DynDNS,
MyDNS.JP

**Region-specific:** PointHQ (NZ), Misaka.io (JP), SiteHost (NZ), BookMyName (DE),
Websupport (SK), MyDevil (PL), Mijn.host (NL), Nodion (DE), Internet.bs (BS),
DurableDNS (US), OpenProvider REST (NL), Alwaysdata (FR), Restena (LU),
Timeweb Cloud (RU), EasyDNS (CA), Arvan (IR), Azion (BR), Sotoon (IR),
Geoscaling, Alviy, Kinghost (BR), ANX (AT), Area7, Artfiles, Beget, DNSServices

### Stub Providers (5)

These providers return an error with guidance. They require external CLI tools
or have no DNS management API.

| Slug | Reason | Alternative |
|------|--------|-------------|
| `df` | DynDNSFree.de has no TXT API | — |
| `hetzner` | Robot doesn't manage DNS | `--dns hetznercloud` |
| `knot` | Requires knsupdate CLI | `--dns nsupdate` |
| `lexicon` | Requires lexicon CLI | Use lexicon CLI directly |
| `nsd` | Requires nsupdate CLI | `--dns nsupdate` |

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

## Development

This project uses [mise](https://mise.jdx.dev/) for task management. All common operations
are available as mise tasks:

```sh
# Build and development
mise run build              # Build the project
mise run test               # Run all tests
mise run lint               # Run clippy linter
mise run format             # Format code with rustfmt
mise run check              # Run format-check, lint, and test

# Provider analysis
mise run providers          # Count implemented vs stub providers

# Packaging
mise run debian-build       # Build .deb package locally
mise run size               # Show binary size
mise run install            # Install to ~/.cargo/bin

# GitHub operations
mise run github-status      # Check GitHub Actions status
mise run github-workflows   # List available workflows
mise run github-releases    # List all releases
mise run github-release     # View latest release

# Release management
mise run release 0.3.0      # Bump version, tag, and push
mise run release-dry 0.3.0  # Show what release would do

# Documentation
mise run doc                # Build and open Rust documentation
```

All tasks are defined in `mise.toml`. Run `mise tasks` to see the complete list.

## License

MIT © 2026
