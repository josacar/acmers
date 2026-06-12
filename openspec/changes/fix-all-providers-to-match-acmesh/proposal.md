# Proposal: Fix all DNS providers to match acme.sh

## Problem Statement

The acmers ACME client had 201 DNS providers implemented, but many did not match the reference implementations in acme.sh. Issues included:
- Wrong API endpoints
- Incorrect authentication methods
- Mismatched environment variable names
- Missing zone resolution logic
- Wrong TTL values
- Missing pagination support
- Incorrect record operations (add/remove)

## Proposed Solution

Conduct a comprehensive audit of all 201 providers against their acme.sh counterparts, then systematically fix each provider to match the reference implementation.

## Approach

1. **Audit Phase**: Spawn 20 parallel agents to audit all providers
2. **Fix Phase**: Fix providers in batches of 6 using parallel agents
3. **Test Phase**: Update mock tests and verify all changes
4. **Implementation Phase**: Implement remaining stub providers

## Scope

### In Scope
- Fix all 201 DNS providers to match acme.sh
- Implement all stub providers
- Update tests to match new implementations
- Fix compiler warnings
- Ensure zero-warning build

### Out of Scope
- Adding new providers not in acme.sh
- Changing the DnsProvider trait interface
- Modifying the ACME protocol engine
- Adding new features (deploy hooks, notifications)

## Success Criteria

- All 201 providers match acme.sh implementations
- All tests passing (83+ tests)
- Zero compiler warnings
- Clean build

## Risks

- **API changes**: Some providers may have updated their APIs since acme.sh was written
- **Authentication**: Some auth methods require complex signing (HMAC, OAuth2, JWT)
- **Testing**: Mock tests need to be updated to match new implementations

## Mitigation

- Use acme.sh as the reference implementation
- Test each provider with mock HTTP server
- Verify with cargo build and cargo test after each batch
