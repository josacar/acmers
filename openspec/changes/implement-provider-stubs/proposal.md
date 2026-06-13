## Why

The README claimed 68 implemented providers with 133 stubs, but investigation revealed 185 were actually implemented. Of the remaining 16 "stubs", 10 had working HTTP APIs and could be fully implemented, expanding coverage from 185 to 195 providers and making acmers usable for more users.

## What Changes

- Implement 10 provider stubs that were returning errors but have working APIs:
  - Curanet (OAuth2 client credentials + REST API for scannet.dk/wannafind.dk/dandomain.dk)
  - MyDNS.JP (HTTP POST with Basic auth to mydns.jp)
  - Linode v3 (legacy API with key in query params)
  - Online.net (REST API with zone versioning workflow)
  - Yandex 360 (OAuth2 access token + Directory API)
  - Yandex PDD (Token header auth + pddimpex2.yandex.net)
  - Google Domains (OAuth Bearer + acmedns.googleapis.com)
  - One.com (username/password session login + admin API)
  - UnoEuro (user/key in URL path + Simply.com API)
  - OpenProvider XML (legacy XML API with username + password hash)
- Update provider counts in README.md (185 → 195 implemented, 16 → 6 stubs)
- Update AGENTS.md provider landscape section
- Keep 6 stubs that require external CLIs or have no API: df, hetzner, knot, lexicon, nsd, samba

## Capabilities

### New Capabilities

- `curanet-provider`: OAuth2 client credentials authentication and REST API integration for Curanet DNS (covers scannet.dk, wannafind.dk, dandomain.dk domains)
- `mydnsjp-provider`: MyDNS.JP provider with Basic auth and HTTP POST-based TXT record management
- `linode-v3-provider`: Linode API v3 support (legacy, distinct from linode_v4) with API key in query parameters
- `online-provider`: Online.net REST API with zone versioning workflow (create temp version, enable, modify, cleanup)
- `yandex360-provider`: Yandex 360 for Business DNS API with OAuth2 access token authentication
- `yandex-provider`: Yandex PDD (personal domain) API with Token header authentication
- `googledomains-provider`: Google Domains ACME DNS API with OAuth2 Bearer token authentication
- `one-provider`: One.com admin API with session-based authentication (username/password → cookie)
- `unoeuro-provider`: UnoEuro/Simply.com API with user/key credentials in URL path
- `openprovider-xml-provider`: OpenProvider legacy XML API with username and password hash authentication

### Modified Capabilities


## Impact

- **Code**: 10 provider files in `src/providers/` expanded from ~28-line stubs to 80-200 line implementations
- **Tests**: All 83 existing tests continue to pass; no new tests added for these providers (would require API credentials)
- **Dependencies**: No new dependencies added
- **APIs**: Each provider uses its respective DNS provider's REST API (no new external dependencies)
- **Documentation**: README.md and AGENTS.md updated with accurate provider counts and stub categories
