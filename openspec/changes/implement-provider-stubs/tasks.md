## 1. Provider Implementations

- [x] 1.1 Implement Curanet provider (OAuth2 client credentials + REST API for scannet.dk/wannafind.dk/dandomain.dk)
- [x] 1.2 Implement MyDNS.JP provider (Basic auth + HTTP POST API)
- [x] 1.3 Implement Linode v3 provider (API key in query params + legacy API)
- [x] 1.4 Implement Online.net provider (Bearer token + zone versioning workflow)
- [x] 1.5 Implement Yandex 360 provider (OAuth access token + Directory API)
- [x] 1.6 Implement Yandex PDD provider (Token header auth + pddimpex2.yandex.net)
- [x] 1.7 Implement Google Domains provider (OAuth Bearer token + acmedns.googleapis.com)
- [x] 1.8 Implement One.com provider (session cookie auth + admin API)
- [x] 1.9 Implement UnoEuro provider (user/key in URL path + Simply.com API)
- [x] 1.10 Implement OpenProvider XML provider (username + password hash in XML body)

## 2. Registry Updates

- [x] 2.1 Update Yandex360 env_vars in mod.rs from CLIENT_ID/CLIENT_SECRET to ACCESS_TOKEN
- [x] 2.2 Verify all 10 providers are properly registered in src/providers/mod.rs

## 3. Documentation Updates

- [x] 3.1 Update README.md provider counts (185 → 195 implemented, 16 → 6 stubs)
- [x] 3.2 Update README.md implemented providers list with new providers
- [x] 3.3 Update README.md stub providers table (remove 10 implemented, keep 6 remaining)
- [x] 3.4 Update AGENTS.md provider landscape section with new counts
- [x] 3.5 Update AGENTS.md stub categories to reflect remaining 6 stubs

## 4. Build and Test Verification

- [x] 4.1 Run cargo build and verify zero errors
- [x] 4.2 Run cargo test and verify all 83 tests pass
- [x] 4.3 Verify zero compiler warnings

## 5. Release

- [x] 5.1 Bump version to 0.2.0 in Cargo.toml
- [x] 5.2 Update debian/changelog with 0.2.0 entry
- [x] 5.3 Commit version bump
- [x] 5.4 Tag v0.2.0 and push to trigger GitHub Actions release
- [x] 5.5 Verify release workflow builds .deb packages for amd64 and arm64

## Summary

- **Total tasks**: 20
- **Completed**: 20
- **Success rate**: 100%
