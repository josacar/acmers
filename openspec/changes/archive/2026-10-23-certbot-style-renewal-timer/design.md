## Context

The repository already ships a Debian package with `acmers.timer` and
`acmers.service`, but its behavior diverges from the certbot package on the
same host: the timer runs once a day, the service has `[Install]` and
`ProtectSystem=strict`, and there is no credential source for unattended DNS
renewal. On the target host an existing acmers certificate lives in
`/home/selu/.acmers`, certbot 5.5.0-3 is installed, and no Cloudflare token is
present.

Certbot's Debian package uses a twice-daily timer with a 12-hour randomized
delay, a static oneshot service with `PrivateTmp=true`, debhelper-managed
enablement, and a cron fallback guarded by `/run/systemd/system`.

## Goals / Non-Goals

**Goals:**
- Make `acmers.timer` and `acmers.service` behave like certbot's units.
- Enable the timer on package installation.
- Load DNS credentials from `/etc/acmers/env` for systemd and cron.
- Prevent overlapping manual and timer renewal runs.
- Document migration from a per-user store to `/var/lib/acmers`.

**Non-Goals:**
- User-level systemd timers.
- Importing certbot's `/etc/letsencrypt` state.
- Log rotation; output goes to the journal.
- Parallel renewal.

## Decisions

### Match certbot's timer rather than inventing a schedule
Use `OnCalendar=*-*-* 00,12:00:00`, `RandomizedDelaySec=43200`, and
`Persistent=true`. Certbot explicitly disables its own random sleep in the
service because the timer already spreads runs. `acmers` has no internal sleep,
so the unit alone provides the spread.

Alternative: keep `OnCalendar=daily` with a one-hour spread. Rejected because it
halves the chances of catching a transient CA or DNS failure before expiry.

### Remove `ProtectSystem=strict`
The service must write configured certificate paths, which commonly live under
`/etc`, and must run reload commands. Certbot's own unit only uses
`PrivateTmp=true`. Keep `PrivateTmp`, `StateDirectory`, and `NoNewPrivileges`,
and drop `ProtectSystem` entirely. A dynamic `ReadWritePaths=` list is not
possible because install paths are per certificate.

### Service is static
Remove `[Install]` from the service. The timer pulls it in on schedule, and the
package enables only the timer. This matches certbot and avoids running renewal
twice at boot.

### Credentials through an environment file
Systemd uses `EnvironmentFile=-/etc/acmers/env`, the same path documented for
the cron fallback. The file is root-owned `0600`; an example is installed at
`/etc/acmers/env.example` so the package never ships a secret.

For cron, add `--env-file <path>` to the CLI instead of sourcing the file from
shell. Parsing `KEY=VALUE` in Rust keeps quoting consistent and avoids executing
file contents. Precedence is `-e` > env file > process environment, so manual
overrides still work.

### Lock file in the acmers home
Add `src/lock.rs` with a PID lock at `<home>/.lock`. Use
`OpenOptions::create_new(true)`; if it exists, read the PID and treat it as stale
when `/proc/<pid>` is absent. The lock is held for the whole `issue` or `renew`
command and removed by a guard on every exit path. This is sufficient because
the timer and cron never use the same home concurrently, and systemd already
prevents overlapping starts of the same oneshot unit.

Alternative: `flock` via `libc`. Rejected to avoid a new direct dependency.

### Drop the unused system user
`debian/postinst` currently creates `_acmers` and chowns `/var/lib/acmers`, but
the service runs as root. Remove the user creation and rely on `StateDirectory`
plus `debian/dirs` for ownership. Running as root is required for reload hooks,
matching certbot.

### debhelper owns the units
Remove unit copies from `override_dh_auto_install` and `debian/install`, and let
`dh_installsystemd` install them and generate the enable/start maintainer
script snippets. This is the same mechanism certbot's package uses.

## Risks / Trade-offs

- **Relaxed sandbox increases service privileges** → The unit runs as root and
  executes user-configured hooks, exactly like certbot. Credentials remain in a
  `0600` file and the unit does not expose them on the command line.
- **Cron and timer could both run if `/run/systemd/system` detection is wrong** →
  The lock makes the second run fail safely instead of corrupting state.
- **Env file parsing differs from shell** → Support simple quoting and document
  that the file is not a shell script.
- **Removing `_acmers` affects upgrades** → Keep `/var/lib/acmers` as
  `root:root 0750`; the service already runs as root.
- **`--env-file` could leak secrets in error output** → Errors report only path
  and line number, never line contents.

## Migration Plan

1. Install the new package; verify `acmers.timer` is enabled.
2. Create `/etc/acmers/env` with `CF_Token`.
3. Move `/home/selu/.acmers` to `/var/lib/acmers` and set `root:root 0750`.
4. Run `acmers renew --all --dry-run --env-file /etc/acmers/env`.
5. Run `systemctl start acmers.service` to reissue the expired certificate.
6. Leave `certbot.timer` enabled as requested; it has no certificates.
7. Rollback: install the previous package, restore the old home, and disable
   `acmers.timer`.

## Open Questions

None. The certificate store location and credential path were chosen by the
user.
