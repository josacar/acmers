## 1. Core Implementation

- [x] 1.1 Replace global `TEST_BASE_URL` (`LazyLock<Mutex<Option<String>>>`) with `thread_local!` + `RefCell<Option<String>>` in `src/http.rs`
- [x] 1.2 Update `set_test_base()` to write to the thread-local cell instead of the global mutex
- [x] 1.3 Update `rewrite_url()` to read from the thread-local cell instead of the global mutex
- [x] 1.4 Add `clear_test_base()` function for explicit thread-local cleanup

## 2. Verification

- [x] 2.1 Run `cargo build` — must compile with zero errors and zero warnings
- [x] 2.2 Run `cargo test` — all 13 provider mock tests must pass (including `test_cloudflare_add_txt`, `test_kas_add_txt`, `test_oci_add_txt`, `test_transip_add_txt`)
- [x] 2.3 Run `cargo test` multiple times to confirm no intermittent failures
