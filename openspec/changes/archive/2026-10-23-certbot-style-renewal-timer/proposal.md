## Why

The Debian package ships a renewal timer that does not match the behavior of
the certbot package it replaces: it runs once a day instead of twice, its
service sandbox makes `/etc` read-only so certificate install paths and reload
hooks fail, and it has no way to load DNS provider credentials for unattended
runs. On a host migrating from certbot, the first timer run would fail to renew
the existing certificate and could not deploy it.

## What Changes

- Align `acmers.timer` with certbot: run at 00:00 and 12:00 with a 12-hour
  randomized delay, `Persistent=true`, and enable it automatically on install.
- Make `acmers.service` static and remove `ProtectSystem=strict` so it can
  write certificates to `/etc` and run reload hooks. Keep `PrivateTmp` and
  `StateDirectory`.
- Load DNS provider credentials from `/etc/acmers/env` through
  `EnvironmentFile`, and ship `/etc/acmers/env.example`.
- Add a certbot-style `/etc/cron.d/acmers` fallback for hosts without systemd.
- Add `--env-file <path>` so the cron fallback and manual runs can load
  `KEY=VALUE` credentials without systemd.
- Add a renewal lock so a manual `acmers renew` cannot overlap the timer.
- Stop creating the unused `_acmers` system user; the service runs as root so
  reload hooks work, matching certbot.
- Document migrating an existing `~/.acmers` store to `/var/lib/acmers`.

## Capabilities

### New Capabilities
- `packaged-renewal-timer`: systemd timer and service semantics, package enablement, sandboxing, and cron fallback.
- `credential-env-file`: loading provider credentials from a file for unattended and cron runs.
- `renewal-locking`: preventing concurrent issuance and renewal runs.

### Modified Capabilities

None.

## Impact

- **Packaging**: `debian/acmers.timer`, `debian/acmers.service`,
  `debian/install`, `debian/rules`, `debian/postinst`, `debian/postrm`,
  `debian/dirs`, new `debian/acmers.cron.d` and `debian/acmers.env.example`.
- **Code**: `src/cli.rs`, `src/main.rs`, new `src/lock.rs`, `src/lib.rs`.
- **Docs**: `README.md`, `debian/acmers.1`.
- **Dependencies**: none added. The lock uses `std::fs` and `/proc`.
- **Non-goals**: user-level timers, tmpfiles-based state creation, log rotation,
  and importing certificates from certbot's `/etc/letsencrypt`.
