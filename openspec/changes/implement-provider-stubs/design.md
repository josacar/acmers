## Context

The acmers project has 201 DNS provider files, but 16 were returning error messages instead of implementing actual API calls. Investigation revealed that 10 of these had working APIs documented in acme.sh that could be ported to Rust, expanding the implemented provider count from 185 to 195.

The 6 remaining stubs (df, hetzner, knot, lexicon, nsd, samba) require external CLI tools or have no DNS management API, making them unsuitable for implementation in a pure-Rust, no-system-calls project.

## Goals / Non-Goals

**Goals:**
- Implement 10 provider stubs with full TXT record add/remove functionality
- Match acme.sh API behavior and authentication methods
- Maintain zero compiler warnings and all existing tests passing
- Update documentation (README.md, AGENTS.md) with accurate provider counts

**Non-Goals:**
- Add integration tests for these providers (would require live API credentials)
- Implement the 6 stubs that require external CLIs
- Refactor existing provider implementations
- Add new dependencies

## Decisions

### Authentication Strategy: Use existing http module patterns

**Decision**: Each provider uses the authentication method from its acme.sh implementation:
- OAuth2 client credentials (Curanet): POST to token endpoint, use Bearer token
- Basic auth (MyDNS.JP): Encode credentials, add Authorization header
- API key in query params (Linode v3): Append to URL
- Bearer token (Online.net, Yandex 360, Google Domains): Add Authorization header
- Token header (Yandex PDD): Custom "Token" header
- Session cookie (One.com): POST login, extract Set-Cookie, use in subsequent requests
- URL path credentials (UnoEuro): Embed user/key in URL path
- XML with credentials (OpenProvider): Include username/hash in XML body

**Rationale**: These methods are proven in acme.sh and match each provider's API requirements. No need to invent new auth patterns.

**Alternatives considered**:
- Abstracting OAuth2 into a shared helper: Rejected because only Curanet uses OAuth2 client credentials flow
- Using a generic "auth provider" trait: Rejected as over-engineering for 10 distinct auth methods

### Zone Resolution: Implement per-provider logic

**Decision**: Each provider implements its own zone resolution logic by:
1. Splitting the domain into parts
2. Trying progressively shorter suffixes (e.g., for _acme-challenge.www.example.com: try www.example.com, then example.com)
3. Making API calls to check if each suffix is a valid zone
4. Returning the first match

**Rationale**: Zone resolution logic varies by API (some return 404, some return error messages, some require checking response content). Provider-specific logic is clearer than trying to abstract.

**Alternatives considered**:
- Shared zone resolution helper with customizable error detection: Rejected because error detection varies too much (HTTP status codes, response body patterns, specific error strings)

### Error Handling: Idempotent remove_txt

**Decision**: All providers implement idempotent `remove_txt`:
- If zone lookup fails, return Ok(())
- If record list fails, return Ok(())
- If record not found, return Ok(())
- Only return error if delete API call fails

**Rationale**: TXT record cleanup must always succeed to prevent certificate renewal failures. This matches the pattern established in existing providers.

**Alternatives considered**:
- Returning errors for all failure cases: Rejected because cleanup failures would block certificate operations

### API Response Parsing: Use serde_json for JSON, string matching for XML/HTML

**Decision**:
- JSON APIs: Parse with serde_json, use json helper functions
- XML APIs (OpenProvider): String matching to extract values
- HTML scraping (not applicable to these 10 providers): Would use regex or string matching

**Rationale**: JSON is structured and serde_json handles it well. XML/HTML in these APIs is simple enough that string matching is clearer than adding an XML parser dependency.

**Alternatives considered**:
- Adding quick-xml dependency for OpenProvider: Rejected because the XML is simple and adding a dependency violates project constraints

## Risks / Trade-offs

**[Risk] API endpoints may change** → Mitigation: Each provider's API is documented in acme.sh wiki. If APIs change, users will report errors and we can update. No automated way to detect API changes without live credentials.

**[Risk] Authentication methods may be deprecated** → Mitigation: OAuth2 and Bearer tokens are industry standards. Session cookies (One.com) are less common but documented in acme.sh. If deprecated, users will encounter auth errors.

**[Risk] Zone resolution may fail for edge cases** → Mitigation: Progressive suffix matching handles most cases (example.com, sub.example.com). Multi-part TLDs (co.uk) are handled by trying all suffixes. Edge cases will surface as user reports.

**[Trade-off] No integration tests** → Accepted because:
- Integration tests require live API credentials for each provider
- Credentials would need to be stored securely in CI
- acme.sh has proven these APIs work
- Unit tests for helper functions (zone resolution, auth) would be valuable but not critical

**[Trade-off] Code duplication across providers** → Accepted because:
- Each provider has unique API quirks
- Abstraction would add complexity without clear benefit
- 10 providers × ~100 lines = ~1000 lines, manageable duplication

## Migration Plan

Not applicable. This is additive work (implementing stubs, not changing existing providers).

**Deployment**: Merge to main, tag release, GitHub Actions builds .deb packages.

**Rollback**: Revert commit if issues discovered. No data migration or state changes.

## Open Questions

None. All 10 providers have clear API documentation in acme.sh, and implementation patterns are established in the codebase.
