# certificate storage

## Purpose

Defines the on-disk certificate and private key layout, file formats, permissions, and atomic replacement behavior.

## Requirements


### Requirement: Certificate file set
After a successful issuance the system SHALL write a complete certificate file set into the certificate directory: `cert.pem` containing only the leaf certificate, `chain.pem` containing the intermediate certificates, `fullchain.pem` containing the leaf followed by the intermediates, and `privkey.pem` containing the private key.

#### Scenario: Issuing a certificate writes all files
- **WHEN** an order is finalized and the certificate is downloaded
- **THEN** the certificate directory contains `cert.pem`, `chain.pem`, `fullchain.pem`, and `privkey.pem`
- **AND** `fullchain.pem` equals `cert.pem` followed by `chain.pem`

#### Scenario: Leaf and chain are separated correctly
- **WHEN** the ACME server returns a PEM response containing a leaf certificate and one or more intermediates
- **THEN** `cert.pem` contains exactly the first certificate block
- **AND** `chain.pem` contains every remaining certificate block in server order

### Requirement: PEM private key output
The system SHALL encode the certificate private key as PKCS#8 PEM in `privkey.pem` and SHALL write `key.pem` as the same PEM content for backward compatibility. The system MUST NOT write DER to either path.

#### Scenario: Private key is valid PEM
- **WHEN** a certificate is issued
- **THEN** `privkey.pem` starts with `-----BEGIN PRIVATE KEY-----` and ends with `-----END PRIVATE KEY-----`
- **AND** `key.pem` contains the same PEM bytes as `privkey.pem`

#### Scenario: PEM body wraps at 64 columns
- **WHEN** the private key DER is base64 encoded for PEM output
- **THEN** each body line contains at most 64 base64 characters

### Requirement: File permissions
The system SHALL create certificate directories with mode `0755`, certificate files with mode `0644`, and private key files with mode `0600`. The account key file MUST also be mode `0600`.

#### Scenario: Private key is owner-only
- **WHEN** `privkey.pem` and `key.pem` are written
- **THEN** their Unix mode is `0600`

#### Scenario: Certificates are world-readable
- **WHEN** `cert.pem`, `chain.pem`, and `fullchain.pem` are written
- **THEN** their Unix mode is `0644`

### Requirement: Atomic file replacement
The system SHALL write each certificate artifact to a temporary file in the destination directory and rename it into place, so a concurrent reload or service read never observes a partial file.

#### Scenario: No partial file is observable
- **WHEN** a certificate artifact is replaced
- **THEN** the final path transitions from the previous complete content to the new complete content in a single rename
- **AND** no temporary file remains after a successful write

#### Scenario: Failed write preserves old file
- **WHEN** writing a new artifact fails before the rename
- **THEN** the previous file at the destination path is unchanged
