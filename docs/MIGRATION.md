# Migrating from certbot (Cloudflare DNS-01)

This guide covers the common case: certbot issuing a Let's Encrypt wildcard
certificate through Cloudflare's DNS plugin, with Caddy or another service
reading the files. acmers does not import certbot's state, so the first step is
a reissue. Keep the certbot certificate until the new one is deployed.

## 1. Inspect certbot

```sh
sudo certbot certificates
sudo grep -E '^(domains|authenticator|installer|server|dns_cloudflare)' \
  /etc/letsencrypt/renewal/*.conf
```

Look for `authenticator = dns-cloudflare` and the file named by
`dns_cloudflare_credentials`. It contains:

```ini
dns_cloudflare_api_token = <token>
```

acmers uses the same token under the name `CF_Token` and the provider slug
`cf`.

## 2. Install acmers and the credentials

```sh
sudo dpkg -i acmers_0.3.0_amd64.deb      # amd64
# sudo dpkg -i acmers_0.3.0_arm64.deb    # arm64

sudo install -d -m 0700 /etc/acmers
sudo install -m 0600 /dev/null /etc/acmers/env
sudoedit /etc/acmers/env
```

```sh
# /etc/acmers/env
CF_Token=<token from certbot's dns-cloudflare.ini>
```

The packaged systemd service loads this file automatically. For manual
commands, pass `--env-file /etc/acmers/env`.

If the Cloudflare credentials use a global API key instead of a scoped token:

```sh
CF_Key=<api key>
CF_Email=<account email>
```

## 3. Reissue

List the apex first. acmers uses the first domain as the certificate name, so
`-d hackinblack.es -d '*.hackinblack.es'` stores state in
`/var/lib/acmers/hackinblack.es/` and covers both the apex and all subdomains.

```sh
sudo acmers issue \
  -d hackinblack.es \
  -d '*.hackinblack.es' \
  --dns cf \
  --email admin@example.com \
  --env-file /etc/acmers/env \
  --dns-servers 1.1.1.1,8.8.8.8 \
  --fullchain-path /etc/caddy/ssl/hackinblack.es/fullchain.pem \
  --key-path /etc/caddy/ssl/hackinblack.es/privkey.pem \
  --reloadcmd /usr/local/sbin/acmers-caddy-reload
```

`--dns-servers` bypasses local resolvers that may have cached NXDOMAIN for the
challenge name. `--dnssleep` remains available as a fixed-wait fallback.

Files are stored under `/var/lib/acmers/<name>/`:

```
cert.pem  chain.pem  fullchain.pem  privkey.pem  key.pem  renewal.json
```

Install paths and hooks are saved in `renewal.json`, so later renewals reuse
them.

## 4. Caddy deployment

Caddy normally runs as the unprivileged `caddy` user and cannot read a
`0600 root:root` key. Use a reload hook that copies the files with the right
ownership and reloads Caddy:

```sh
sudo tee /usr/local/sbin/acmers-caddy-reload >/dev/null <<'EOF'
#!/bin/sh
set -eu
install -d -m 0750 /etc/caddy/ssl/hackinblack.es
chown root:caddy /etc/caddy/ssl/hackinblack.es
chmod 0750 /etc/caddy/ssl/hackinblack.es
chown root:caddy /etc/caddy/ssl/hackinblack.es/privkey.pem
chmod 0640 /etc/caddy/ssl/hackinblack.es/privkey.pem
systemctl reload caddy
EOF
sudo chmod 0755 /usr/local/sbin/acmers-caddy-reload
```

```caddyfile
hackinblack.es, *.hackinblack.es {
    tls /etc/caddy/ssl/hackinblack.es/fullchain.pem /etc/caddy/ssl/hackinblack.es/privkey.pem
}
```

```sh
sudo caddy validate --config /etc/caddy/Caddyfile
sudo systemctl reload caddy
```

For a service other than Caddy, use its real certificate and key paths:

```sh
sudo acmers renew -d hackinblack.es \
  --fullchain-path /etc/nginx/ssl/hackinblack.es/fullchain.pem \
  --key-path /etc/nginx/ssl/hackinblack.es/privkey.pem \
  --reloadcmd 'systemctl reload nginx'
```

## 5. Verify before disabling certbot

```sh
sudo acmers certificates
sudo acmers renew --all --dry-run --env-file /etc/acmers/env
sudo systemctl start acmers.service
sudo journalctl -u acmers.service -n 50 --no-pager

sudo openssl x509 -in /var/lib/acmers/hackinblack.es/cert.pem \
  -noout -subject -issuer -enddate -ext subjectAltName
```

The dry run uses staging and does not install files or run hooks.

## 6. Stop certbot renewals

If acmers now manages every certificate:

```sh
sudo systemctl disable --now certbot.timer
sudo rm -f /etc/cron.d/certbot
```

If certbot still manages other certificates, remove only the migrated renewal
configuration:

```sh
sudo mv /etc/letsencrypt/renewal/hackinblack.es.conf \
  /root/hackinblack.es.conf.bak
```

Do not delete certbot's `live/` files until the service is using the acmers
paths or deployed copies.

## 7. Automatic renewal

The Debian package enables `acmers.timer`:

- runs at 00:00 and 12:00 with up to 12 hours of random delay,
- `Persistent=true`, so a missed run catches up after boot,
- executes `/usr/bin/acmers renew --all` as root,
- loads `/etc/acmers/env`,
- runs the stored `--reloadcmd` only when a certificate was actually renewed.

State lives in `/var/lib/acmers`. User-level runs use `~/.acmers`; to move an
existing user store to the system service:

```sh
sudo install -d -m 0750 -o root -g root /var/lib/acmers
sudo cp -a ~/.acmers/. /var/lib/acmers/
sudo chown -R root:root /var/lib/acmers
sudo chmod -R go-rwx /var/lib/acmers
sudo systemctl start acmers.service
```

## Other CAs (EAB)

The Cloudflare walkthrough above works with any CA preset. Some CAs require
External Account Binding (EAB): supply the key identifier and HMAC key from the
CA dashboard on the first registration. ZeroSSL example:

```sh
sudo acmers register --email admin@example.com --ca zerossl \
  --eab-kid 'kid-from-zerossl' \
  --eab-hmac-key 'hmac-key-from-zerossl'
```

`ACME_EAB_KID` and `ACME_EAB_HMAC_KEY` can be used instead of the flags to keep
secrets out of shell history. EAB is only needed when the account is created:
renewals reuse the stored account and need neither. EAB secrets are never
written to disk.

The full preset list is in the README. `--server URL` still accepts an
arbitrary directory and overrides the preset URL, and the selected CA is stored
in `renewal.json` so `acmers show -d example.com` reports it.

## Caveats

- acmers issues ECDSA P-256 certificates. If a service requires RSA, it is not
  supported yet.
- acmers does not implement certbot's nginx/apache/webroot plugins. DNS-01 and
  standalone HTTP-01 are available.
- CAs requiring External Account Binding (ZeroSSL, Google Trust Services,
  SSL.com, Sectigo) are supported; pass `--eab-kid` and `--eab-hmac-key` once
  during registration.
- Buypass discontinued TLS/SSL issuance in October 2025; its presets remain for
  existing accounts/test endpoints but cannot issue new certificates.
- certbot's ACME account and `/etc/letsencrypt` state are not imported.
- Cloudflare's certbot token works unchanged; only the variable name differs
  (`dns_cloudflare_api_token` → `CF_Token`).
