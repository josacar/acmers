## Context

`src/http.rs` exposes a `TEST_BASE_URL` global (`LazyLock<Mutex<Option<String>>>`) that, when set via `set_test_base()`, rewrites all outgoing HTTP URLs to a local mock server. Provider mock tests in `tests/provider_mock.rs` each create their own `MockServer` on an ephemeral port and call `set_test_base()` to point HTTP requests at it.

The problem: Rust's test runner runs tests in parallel across threads by default. Since `TEST_BASE_URL` is a process-global static, concurrent tests overwrite each other's base URL. A request from test A can land on test B's mock server, which returns 404 because it doesn't recognize the request pattern. Four tests (Cloudflare, KAS, OCI, TransIP) are consistently affected.

## Goals / Non-Goals

**Goals:**
- Eliminate the race condition so all provider mock tests pass reliably with parallel execution
- Maintain the existing `set_test_base()` API surface — no changes to test code
- Keep the fix confined to `src/http.rs`

**Non-Goals:**
- Changing provider implementations
- Changing mock server infrastructure
- Adding new dependencies
- Serializing test execution (no `--test-threads=1`)

## Decisions

### Decision: Use `std::cell::RefCell` with `thread_local!` instead of global `LazyLock<Mutex<>>`

**Rationale:** `thread_local!` gives each test thread its own copy of the base URL. `RefCell` provides interior mutability within the thread without synchronization overhead. This is sound because:
- Each test runs in a single thread (no async)
- Mock server HTTP requests happen on the same thread that called `set_test_base()`
- No `Send + Sync` required for test-only code

**Alternative considered:** `std::sync::Arc<Mutex<>>` — would need each test to pass around an Arc, requiring changes to the `DnsProvider` trait and all 201 providers. Too invasive.

**Alternative considered:** Run tests with `--test-threads=1` — masks the problem instead of fixing it; slows down CI.

### Decision: Keep `set_test_base()` signature unchanged

The function signature `pub fn set_test_base(url: &str)` stays. Internally it writes to the thread-local cell instead of the global mutex. No test code changes required.

### Decision: Expose `clear_test_base()` for cleanup

Add `pub fn clear_test_base()` to reset the thread-local after each test. While `MockServer::drop` stops the server thread, the base URL pointer lingers. Explicit cleanup prevents cross-contamination if tests share threads (unlikely but defensive).

## Risks / Trade-offs

- **[Risk] ureq's Agent is global** → The `CLIENT` static (`LazyLock<HttpClient>`) is still process-global. If ureq's `Agent` caches connections across test threads, a stale connection to a stopped mock server could cause spurious "connection refused" errors. **Mitigation:** ureq 3's `Agent` creates fresh connections per request by default; the mock server sets `Connection: close`.

- **[Risk] Thread pooling** → If the test runner replicates threads via a thread pool, a thread could serve multiple tests sequentially with stale `TEST_BASE_URL` from the previous test. **Mitigation:** `clear_test_base()` should be called in test cleanup. As a belt-and-suspenders measure, `set_test_base()` overwrites any previous value.

- **[Trade-off] Test-only code in production module** → `TEST_BASE_URL` and `set_test_base()` live in `src/http.rs` with `#[cfg(test)]` or always-compiled. Currently they are always compiled (no `#[cfg(test)]` gating). We keep this pattern to avoid conditional compilation complexity.
