## ADDED Requirements

### Requirement: Twice-daily timer
The packaged `acmers.timer` SHALL run at 00:00 and 12:00 each day, with a randomized delay of up to 12 hours and `Persistent=true`, matching the certbot Debian package.

#### Scenario: Timer has two daily triggers
- **WHEN** `acmers.timer` is inspected
- **THEN** it contains `OnCalendar=*-*-* 00,12:00:00`
- **AND** it contains `RandomizedDelaySec=43200`
- **AND** it contains `Persistent=true`

#### Scenario: Missed run is caught up
- **WHEN** the host was powered off across a scheduled trigger
- **THEN** `Persistent=true` causes the timer to run when the host returns

### Requirement: Static renewal service
The packaged `acmers.service` SHALL be a oneshot unit that runs `renew --all`, SHALL have no `[Install]` section, and SHALL be started only by the timer or manually.

#### Scenario: Service is not enabled at boot
- **WHEN** the package is installed
- **THEN** the timer is enabled
- **AND** the service is not separately enabled

#### Scenario: Service runs bulk renewal
- **WHEN** `systemctl start acmers.service` runs
- **THEN** the executed command is `/usr/bin/acmers renew --all`

### Requirement: Service can deploy certificates
The service unit SHALL NOT make `/etc` read-only, so configured certificate paths and reload hooks work under systemd.

#### Scenario: Install path under /etc
- **WHEN** a certificate is configured with `--fullchain-path /etc/nginx/ssl/fullchain.pem`
- **AND** renewal runs from `acmers.service`
- **THEN** the file is written successfully

#### Scenario: Reload hook runs
- **WHEN** renewal runs from `acmers.service` with `--reloadcmd`
- **THEN** the hook is executed

### Requirement: Service state and isolation
The service SHALL use `StateDirectory=acmers` with mode `0750`, `PrivateTmp=true`, and SHALL run as root so deployment and reload hooks can manage system services.

#### Scenario: State directory exists
- **WHEN** the service starts
- **THEN** `/var/lib/acmers` exists with mode `0750`

#### Scenario: No unused system user
- **WHEN** the package is installed
- **THEN** it does not create an `_acmers` system user

### Requirement: Timer enablement on install
The Debian package SHALL enable and start `acmers.timer` during configuration using the debhelper-generated systemd maintainer scripts.

#### Scenario: Fresh install
- **WHEN** the package is installed on a systemd host
- **THEN** `acmers.timer` is enabled
- **AND** `acmers.timer` is started

### Requirement: Non-systemd cron fallback
The package SHALL install `/etc/cron.d/acmers`, which runs renewal twice daily with a random sleep only when `/run/systemd/system` does not exist.

#### Scenario: Systemd host skips cron
- **WHEN** `/run/systemd/system` exists
- **THEN** the cron entry does not run renewal

#### Scenario: Non-systemd host uses cron
- **WHEN** `/run/systemd/system` does not exist
- **THEN** the cron entry runs `acmers renew --all` with credentials from `/etc/acmers/env`
