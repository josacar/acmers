use std::collections::HashMap;

use acmers::acme;
use acmers::base64;
use acmers::cli;
use acmers::config;
use acmers::crypto;
use acmers::error::Error;
use acmers::http;
use acmers::json;
use acmers::providers;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = match cli::parse(&args[1..]) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = run(cmd) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(cmd: cli::Cmd) -> Result<(), Error> {
    match cmd {
        cli::Cmd::Issue { domains, provider, email, test, standalone, env_overrides } => {
            cmd_issue(&domains, &provider, email.as_deref(), test, standalone, &env_overrides)
        }
        cli::Cmd::Renew { domain, test } => cmd_renew(&domain, test),
        cli::Cmd::Revoke { domain } => cmd_revoke(&domain),
        cli::Cmd::ListProviders => cmd_list_providers(),
        cli::Cmd::Register { email, test } => cmd_register(&email, test),
        cli::Cmd::Cron => cmd_cron(),
    }
}

fn new_nonce(nonce_url: &str) -> Result<String, Error> {
    let resp = http::head(nonce_url)
        .map_err(|e| Error::Config(format!("get nonce: {e}")))?;
    resp.headers.get("replay-nonce")
        .or_else(|| resp.headers.get("Replay-Nonce"))
        .cloned()
        .ok_or_else(|| Error::Acme {
            status: resp.status,
            detail: "no Replay-Nonce in response".into(),
            error_type: "missing_nonce".into(),
        })
}

fn mk_nonce_fn<'a>(nonce_url: &'a str) -> impl FnMut() -> Result<String, Error> + 'a {
    move || new_nonce(nonce_url)
}

fn cmd_issue(
    domains: &[String],
    provider_slug: &str,
    email: Option<&str>,
    test: bool,
    standalone: bool,
    env_overrides: &HashMap<String, String>,
) -> Result<(), Error> {
    let mut config = config::Config::load()?;
    if test {
        config.server = "https://acme-staging-v02.api.letsencrypt.org/directory".to_string();
    }

    let email = email.ok_or_else(|| Error::Config("email required (use --email)".into()))?;

    let account = acme::account::load_or_register(&config, email)?;
    if !account.server.is_empty() {
        config.server = account.server.clone();
    } else if test {
        config.server = "https://acme-staging-v02.api.letsencrypt.org/directory".to_string();
    }
    let account_key = crypto::load_key_from_der(&base64::decode(&account.key_b64)
        .map_err(|e| Error::Config(format!("decode account key: {e}")))?)?;

    let directory = acme::directory::fetch(&config.server)?;
    let mut get_nonce = mk_nonce_fn(&directory.new_nonce);

    let nonce = get_nonce()?;
    let order = acme::order::create_order(
        domains, &account.url, &directory.new_order, &account_key, &nonce,
    )?;
    eprintln!("order created: {}", order.url);

    let auths = acme::challenge::get_authorizations(
        &account.url, &order.authorizations, &account_key, &mut get_nonce,
    )?;

    let main_domain = &domains[0];

    if standalone {
        let mut challenge_pairs: Vec<(String, String)> = Vec::new();
        let mut challenge_urls: Vec<(String, String)> = Vec::new();
        for auth in &auths {
            let domain = &auth.identifier.value;
            let ch = auth.challenges.iter()
                .find(|c| c.typ == "http-01")
                .ok_or_else(|| Error::Dns(format!("no http-01 challenge for {domain}")))?;
            let token = ch.token.as_deref().unwrap_or("");
            let key_auth = ch.key_authorization(&account.jwk_thumbprint);
            challenge_pairs.push((token.to_string(), key_auth));
            challenge_urls.push((ch.url.clone(), domain.clone()));
        }

        let _server = acme::challenge::start_http_challenges(&challenge_pairs);

        for (url, domain) in &challenge_urls {
            eprintln!("signaling ACME server for {domain}...");
            acme::challenge::respond_to_challenge(
                url, &account.url, &account_key, &mut get_nonce,
            )?;

            eprintln!("waiting for HTTP-01 validation of {domain}...");
            let result = acme::challenge::poll_challenge(
                url, &account.url, &account_key, &mut get_nonce,
            );

            result?;
            eprintln!("{domain} validated!");
        }
    } else {
        let provider_meta = providers::find(provider_slug)
            .ok_or_else(|| Error::Config(format!("unknown provider: {provider_slug}")))?;

        let mut env = config::read_env_vars(provider_meta.env_vars);
        for (k, v) in env_overrides {
            env.insert(k.clone(), v.clone());
        }

        let provider = (provider_meta.create)(&env)?;

        for auth in &auths {
            let domain = &auth.identifier.value;
            let dns_challenge = auth.challenges.iter()
                .find(|c| c.typ == "dns-01")
                .ok_or_else(|| Error::Dns(format!("no dns-01 challenge for {domain}")))?;
            let token = dns_challenge.token.as_deref()
                .ok_or_else(|| Error::Dns(format!("no token for {domain}")))?;

            let txt_value = acme::account::dns_txt_value(token, &account_key.jwk_thumbprint);
            let challenge_domain = format!("_acme-challenge.{domain}");

            eprintln!("adding TXT record {challenge_domain} = {txt_value}");
            provider.add_txt(main_domain, &challenge_domain, &txt_value)?;

            eprintln!("signaling ACME server...");
            acme::challenge::respond_to_challenge(
                &dns_challenge.url, &account.url, &account_key, &mut get_nonce,
            )?;

            eprintln!("waiting for validation...");
            let result = acme::challenge::poll_challenge(
                &dns_challenge.url, &account.url, &account_key, &mut get_nonce,
            );

            eprintln!("removing TXT record...");
            let _ = provider.remove_txt(main_domain, &challenge_domain, &txt_value);

            result?;
            eprintln!("{domain} validated!");
        }
    }

    eprintln!("finalizing order...");
    let csr = crypto::create_csr(domains, &account_key.pkcs8_bytes)?;
    let cert_url = acme::order::finalize_order(
        &csr, &order.finalize, &account.url, &account_key, &mut get_nonce,
    )?;

    eprintln!("downloading certificate...");
    let cert_pem = acme::order::download_cert(
        &cert_url, &account.url, &account_key, &mut get_nonce,
    )?;

    let domain_dir = config.domain_dir(main_domain);
    std::fs::create_dir_all(&domain_dir)?;
    std::fs::write(config.cert_file(main_domain), &cert_pem)?;
    std::fs::write(config.key_file(main_domain), &account_key.pkcs8_bytes)?;
    std::fs::write(config.fullchain_file(main_domain), &cert_pem)?;

    save_renewal_config(&config, main_domain, provider_slug, email, test, standalone)?;
    eprintln!("certificate saved to {:?}", config.cert_file(main_domain));
    Ok(())
}

fn cmd_renew(domain: &str, test: bool) -> Result<(), Error> {
    let mut config = config::Config::load()?;
    if test {
        config.server = "https://acme-staging-v02.api.letsencrypt.org/directory".to_string();
    }

    let renewal_file = config.domain_dir(domain).join("renewal.json");
    let renewal_data = std::fs::read_to_string(&renewal_file)?;
    let v: serde_json::Value = serde_json::from_str(&renewal_data)
        .map_err(|e| Error::Config(format!("parse renewal config: {e}")))?;

    let email = json::get_string_required(&v, &["email"])?.to_string();
    let provider_slug = json::get_string_required(&v, &["provider"])?.to_string();
    let standalone = v.get("standalone").and_then(|s| s.as_bool()).unwrap_or(false);

    let cert_file = config.cert_file(domain);
    if cert_file.exists() {
        let cert_data = std::fs::read_to_string(&cert_file)?;
        let remaining = days_until_expiry(&cert_data);
        eprintln!("{domain}: {remaining} days until expiry");
    }

    cmd_issue(&[domain.to_string()], &provider_slug, Some(&email), test, standalone, &HashMap::new())
}

fn cmd_revoke(domain: &str) -> Result<(), Error> {
    let mut config = config::Config::load()?;
    let cert_pem = std::fs::read_to_string(config.cert_file(domain))?;

    let account_data = std::fs::read_to_string(config.account_file())?;
    let v: serde_json::Value = serde_json::from_str(&account_data)
        .map_err(|e| Error::Config(format!("parse account: {e}")))?;
    let account_url = json::get_string_required(&v, &["url"])?.to_string();
    let account_pkcs8_b64 = json::get_string_required(&v, &["key"])?.to_string();
    if let Some(s) = json::get_string(&v, &["server"]) {
        config.server = s.to_string();
    }
    let account_key = crypto::load_key_from_der(
        &base64::decode(&account_pkcs8_b64)
            .map_err(|e| Error::Crypto(format!("decode key: {e}")))?,
    )?;

    let directory = acme::directory::fetch(&config.server)?;
    let mut get_nonce = mk_nonce_fn(&directory.new_nonce);
    let nonce = get_nonce()?;

    let payload = serde_json::json!({
        "certificate": cert_pem,
        "reason": 1,
    });

    let jws = crypto::sign_jws(
        &serde_json::to_vec(&payload).unwrap(),
        &account_key.key_pair,
        &crypto::KidOrJwk::Kid(account_url),
        &nonce,
        &directory.revoke_cert,
    )?;

    let resp = http::post(
        &directory.revoke_cert,
        &serde_json::to_vec(&jws).unwrap(),
        "application/jose+json",
        &[],
    )
    .map_err(|e| Error::Acme { status: 0, detail: e, error_type: "http".into() })?;

    if resp.status == 200 {
        eprintln!("certificate for {domain} revoked");
    } else {
        eprintln!("revoke returned status {}", resp.status);
    }
    Ok(())
}

fn cmd_list_providers() -> Result<(), Error> {
    println!("Available DNS providers:");
    for meta in providers::list() {
        let env = meta.env_vars.join(", ");
        println!("  {:<20} env: {env}", meta.slug);
    }
    Ok(())
}

fn cmd_register(email: &str, test: bool) -> Result<(), Error> {
    let mut config = config::Config::load()?;
    if test {
        config.server = "https://acme-staging-v02.api.letsencrypt.org/directory".to_string();
    }
    let account = acme::account::load_or_register(&config, email)?;
    eprintln!("registered: {}", account.url);
    Ok(())
}

fn cmd_cron() -> Result<(), Error> {
    let config = config::Config::load()?;
    let home = &config.home;
    if !home.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(home)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let domain = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if domain.contains('.') {
            let cert_file = config.cert_file(domain);
            if cert_file.exists() {
                let data = std::fs::read_to_string(&cert_file)?;
                if days_until_expiry(&data) < 30 {
                    eprintln!("renewing {domain}...");
                    cmd_renew(domain, false)?;
                }
            }
        }
    }
    Ok(())
}

fn save_renewal_config(
    config: &config::Config,
    domain: &str,
    provider: &str,
    email: &str,
    test: bool,
    standalone: bool,
) -> Result<(), Error> {
    let renewal = serde_json::json!({
        "email": email,
        "provider": provider,
        "test": test,
        "standalone": standalone,
        "server": config.server,
    });
    let path = config.domain_dir(domain).join("renewal.json");
    std::fs::write(path, serde_json::to_string_pretty(&renewal).unwrap())?;
    Ok(())
}

fn days_until_expiry(cert_pem: &str) -> u64 {
    let der = match pem_to_der(cert_pem.as_bytes()) {
        Ok(d) => d,
        Err(_) => return 90,
    };
    use time::OffsetDateTime;
    match x509_parser::parse_x509_certificate(&der) {
        Ok((_, cert)) => {
            let not_after = cert.tbs_certificate.validity.not_after;
            let expiry = OffsetDateTime::from_unix_timestamp(not_after.timestamp()).unwrap_or(OffsetDateTime::now_utc());
            let now = OffsetDateTime::now_utc();
            let secs = expiry - now;
            let days = secs.whole_days();
            if days < 0 { 0 } else { days as u64 }
        }
        Err(_) => 90,
    }
}

fn pem_to_der(pem: &[u8]) -> Result<Vec<u8>, String> {
    let s = std::str::from_utf8(pem).map_err(|_| "PEM not UTF-8".to_string())?;
    let lines: Vec<&str> = s.lines()
        .skip_while(|l| !l.starts_with("-----BEGIN"))
        .skip(1)
        .take_while(|l| !l.starts_with("-----END"))
        .collect();
    let b64: String = lines.join("");
    base64::decode(&b64).map_err(|e| format!("PEM decode: {e}"))
}
