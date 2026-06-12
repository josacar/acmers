## Why

Provider mock tests (`tests/provider_mock.rs`) have intermittent failures due to a race condition: `TEST_BASE_URL` in `src/http.rs` is a global `LazyLock<Mutex<Option<String>>>` shared across all test threads. When tests run in parallel, each test's `set_test_base()` overwrites the URL, causing requests to go to wrong mock servers and return 404. This affects 4 of 13 provider mock tests (Cloudflare, KAS, OCI, TransIP).

## What Changes

- Replace global `TEST_BASE_URL` with a thread-local storage mechanism so each test thread maintains its own mock server URL
- Each test's `set_test_base()` call affects only requests made from the calling thread

## Capabilities

### New Capabilities
- `thread-local-test-base`: Thread-local HTTP base URL override for test isolation

### Modified Capabilities
<!-- No existing capability requirements change -->

## Impact

- `src/http.rs`: Change `TEST_BASE_URL` from `LazyLock<Mutex<Option<String>>>` to a thread-local cell
- No provider code changes needed
- No API or CLI changes
- No dependency changes
