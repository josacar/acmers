## 1. Systemd Units

- [x] 1.1 Change `acmers.timer` to `OnCalendar=*-`-`* 00,12:00:00`, `RandomizedDelaySec=43200`, `Persistent=true`
- [x] 1.2 Remove `[Install]` from `acmers.service`
- [x] 1.3 Remove `ProtectSystem=strict` from `acmers.service`
- [x] 1.4 Add `EnvironmentFile=-/etc/acmers/env` and keep `StateDirectory`, `PrivateTmp`, `NoNewPrivileges`

## 2. Debian Packaging

- [x] 2.1 Remove manual unit copies from `debian/rules` and `debian/install` so `dh_installsystemd` owns them
- [x] 2.2 Remove `_acmers` user creation and chown from `debian/postinst` and `debian/postrm`
- [x] 2.3 Add `/etc/acmers` to `debian/dirs` and install `acmers.env.example`
- [x] 2.4 Add `/etc/cron.d/acmers` with the non-systemd guard and random sleep
- [x] 2.5 Verify the generated package enables and starts `acmers.timer`

## 3. Environment File Loading

- [x] 3.1 Add `--env-file <path>` to `issue` and `renew`
- [x] 3.2 Parse blank lines, comments, `KEY=VALUE`, and quoted values
- [x] 3.3 Report malformed lines by path and line number without contents
- [x] 3.4 Apply precedence `-e` > env file > process environment
- [x] 3.5 Use `--env-file /etc/acmers/env` in the cron entry

## 4. Renewal Lock

- [x] 4.1 Add `src/lock.rs` with a PID lock at `<home>/.lock`
- [x] 4.2 Acquire the lock in `issue` and `renew`
- [x] 4.3 Release the lock on success, error, and panic-free early returns
- [x] 4.4 Remove a stale lock when the recorded PID is not alive

## 5. Documentation

- [x] 5.1 Document `/etc/acmers/env`, the timer, and the cron fallback in README
- [x] 5.2 Document migrating `~/.acmers` to `/var/lib/acmers`
- [x] 5.3 Update the manpage for the timer, env file, and service

## 6. Tests and Verification

- [x] 6.1 Unit-test env file parsing and precedence
- [x] 6.2 Unit-test lock acquisition, contention, release, and stale recovery
- [x] 6.3 Run `cargo fmt --all -- --check`, clippy with `-D warnings`, and the full test suite
- [x] 6.4 Run `systemd-analyze verify` on both units
- [x] 6.5 Build the `.deb` and inspect installed files and maintainer scripts
