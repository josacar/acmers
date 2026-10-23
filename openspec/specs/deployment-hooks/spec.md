# deployment hooks

## Purpose

Defines installation of certificate artifacts to service paths and execution of pre, post, deploy, and reload hooks.

## Requirements


### Requirement: Install path options
The `issue` and `renew` commands SHALL accept `--cert-path`, `--key-path`, `--fullchain-path`, and `--chain-path`, each naming a destination file for the corresponding artifact.

#### Scenario: Copy certificate files
- **WHEN** a certificate is issued with all four install paths
- **THEN** each source artifact is copied to its destination
- **AND** the copied certificate files have mode `0644`
- **AND** the copied private key has mode `0600`

#### Scenario: Missing install path is skipped
- **WHEN** a certificate is issued with only `--fullchain-path`
- **THEN** only the full chain is copied
- **AND** no error is raised for the unspecified paths

### Requirement: Atomic deployment
The system SHALL copy each artifact to a temporary file next to its destination and rename it into place. Deployment MUST occur only after all requested artifacts have been written successfully.

#### Scenario: Deployment is all-or-nothing per file
- **WHEN** a copy to a destination path fails
- **THEN** the previous file at that destination is unchanged
- **AND** the command reports an error

### Requirement: Reload and deploy hooks
The system SHALL accept `--reloadcmd` and `--deploy-hook` on `issue` and `renew`. Both SHALL run only when a certificate was actually issued or renewed, and `--reloadcmd` SHALL run after install paths are deployed. Hook commands SHALL be executed through `/bin/sh -c`.

#### Scenario: Reload after renewal
- **WHEN** a certificate is renewed with `--reloadcmd "systemctl reload nginx"`
- **THEN** the command runs after the certificate files are installed

#### Scenario: No hook when not due
- **WHEN** `renew` skips a certificate that is not due
- **THEN** `--reloadcmd`, `--deploy-hook`, `--pre-hook`, and `--post-hook` do not run for that certificate

#### Scenario: Dry run runs no hooks
- **WHEN** renewal is performed with `--dry-run`
- **THEN** no reload or deploy hook is executed

### Requirement: Pre and post hooks
The system SHALL accept `--pre-hook` and `--post-hook`. The pre-hook SHALL run once before the first ACME order of a command, and the post-hook SHALL run once after all orders complete, including when an order fails.

#### Scenario: Pre-hook runs once
- **WHEN** `renew --all` processes three certificates with `--pre-hook` set
- **THEN** the pre-hook runs exactly once

#### Scenario: Post-hook runs after failure
- **WHEN** an order fails during a run with `--post-hook` set
- **THEN** the post-hook still runs

### Requirement: Hook environment
The system SHALL provide `RENEWED_LINEAGE` set to the certificate directory and `RENEWED_DOMAINS` set to a space-separated list of the certificate's domains when running deploy and reload hooks.

#### Scenario: Hook can locate the certificate
- **WHEN** a deploy hook runs after renewal of a certificate for two domains
- **THEN** `RENEWED_LINEAGE` points at the certificate directory
- **AND** `RENEWED_DOMAINS` contains both domain names

### Requirement: Hook failures are reported
A non-zero exit status from a reload or deploy hook SHALL be reported as an error and SHALL cause the command to exit non-zero, while leaving the issued certificate in place.

#### Scenario: Reload command fails
- **WHEN** `--reloadcmd` exits non-zero
- **THEN** the certificate remains stored
- **AND** the command reports the hook failure and exits non-zero

### Requirement: Hook persistence
The system SHALL persist install paths and hook commands in the renewal configuration so that later `renew` runs apply them without repeating the flags.

#### Scenario: Flags are reused on renewal
- **WHEN** a certificate was issued with `--fullchain-path` and `--reloadcmd`
- **THEN** `renew -d <domain>` copies to that path and runs that command without the flags being repeated
