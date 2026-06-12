# Thread-Local Test Base

## Purpose

Provides thread-local HTTP URL rewriting for provider mock testing, ensuring tests running in parallel do not interfere with each other's mock server URLs.

## Requirements

### Requirement: Test base URL is thread-local

The test HTTP base URL override SHALL be stored in thread-local storage so that concurrent test threads do not interfere with each other's mock server URLs.

#### Scenario: Parallel tests with different mock servers

- **WHEN** two tests run concurrently in separate threads, each creating its own MockServer and calling `set_test_base()` with different URLs
- **THEN** HTTP requests from each test SHALL be routed to that test's own mock server, not the other test's

#### Scenario: set_test_base affects only current thread

- **WHEN** `set_test_base("http://127.0.0.1:10001")` is called on thread A
- **THEN** HTTP requests on thread A SHALL be rewritten to `http://127.0.0.1:10001` paths
- **AND** HTTP requests on thread B (which called `set_test_base("http://127.0.0.1:10002")`) SHALL be rewritten to `http://127.0.0.1:10002` paths

### Requirement: URL rewriting preserves path structure

The URL rewrite logic SHALL extract the path from the original URL (scheme + host + path) and append it to the thread-local base URL exactly as before.

#### Scenario: Full URL is rewritten correctly

- **WHEN** `TEST_BASE_URL` is set to `http://127.0.0.1:9000`
- **AND** a request is made to `https://api.cloudflare.com/client/v4/zones/zone123`
- **THEN** the actual HTTP request SHALL target `http://127.0.0.1:9000/client/v4/zones/zone123`
