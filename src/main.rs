use std::collections::HashMap;
use std::time::Duration;

use acmers::acme;
use acmers::base64;
use acmers::ca;
use acmers::cli;
use acmers::config::{self, Config};
use acmers::crypto;
use acmers::deploy;
use acmers::dns;
use acmers::envfile;
use acmers::error::Error;
use acmers::http;
use acmers::lock;
use acmers::providers;
use acmers::renewal::RenewalConfig;
use acmers::storage;

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
        cli::Cmd::Issue(args) => cmd_issue(args),
        cli::Cmd::Renew(args) => cmd_renew(args),
        cli::Cmd::Revoke {
            domain,
            reason,
            delete_after_revoke,
        } => cmd_revoke(&domain, reason.as_deref(), delete_after_revoke),
        cli::Cmd::ListProviders => cmd_list_providers(),
        cli::Cmd::Register {
            email,
            server,
            ca,
            eab_kid,
            eab_hmac_key,
            test,
        } => cmd_register(
            &email,
            ca.as_deref(),
            server.as_deref(),
            test,
            eab_kid.as_deref(),
            eab_hmac_key.as_deref(),
        ),
        cli::Cmd::Cron => cmd_renew(cli::RenewArgs {
            domain: None,
            all: true,
            server: None,
            test: false,
            force: false,
            dry_run: false,
            days: None,
            keep_until_expiring: false,
            preferred_chain: None,
            env_file: None,
            install: cli::InstallOptions::default(),
        }),
        cli::Cmd::Certificates => cmd_certificates(),
        cli::Cmd::Show { domain } => cmd_show(&domain),
        cli::Cmd::ShowAccount { server, test } => cmd_show_account(server.as_deref(), test),
        cli::Cmd::Delete { domain } => cmd_delete(&domain),
    }
}

fn new_nonce(nonce_url: &str) -> Result<String, Error> {
    let resp = http::head(nonce_url).map_err(|e| Error::Config(format!("get nonce: {e}")))?;
    resp.headers
        .get("replay-nonce")
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

fn cmd_issue(args: cli::IssueArgs) -> Result<(), Error> {
    let config = Config::load()?;
    let _lock = lock::Lock::acquire(&config.home)?;
    let env_overrides = envfile::merge(
        args.env_file.as_deref().map(std::path::Path::new),
        &args.env_overrides,
    )?;
    let resolved = ca::resolve(args.ca.as_deref(), args.server.as_deref(), args.test)?;
    let eab = acme::account::EabCredentials::from_options(
        args.eab_kid.as_deref(),
        args.eab_hmac_key.as_deref(),
    )?;
    let server = resolved.directory;
    let email = args
        .email
        .ok_or_else(|| Error::Config("email required (use --email)".into()))?;

    let mut rc = RenewalConfig::new(args.domains, args.provider, email, server);
    rc.ca = resolved.name.map(|name| name.to_string());
    rc.staging = args.test || rc.server == ca::staging().directory;
    rc.challenge = if args.standalone {
        "http-01".to_string()
    } else {
        "dns-01".to_string()
    };
    rc.dnssleep = args.dnssleep;
    rc.dns_servers = args.dns_servers;
    rc.dns_timeout = args.dns_timeout.unwrap_or(120);
    rc.preferred_chain = args.preferred_chain;
    apply_install(&mut rc, &args.install);

    issue_certificate(&config, &rc, &env_overrides, true, eab.as_ref())
}

fn apply_install(rc: &mut RenewalConfig, install: &cli::InstallOptions) {
    if install.cert_path.is_some() {
        rc.cert_path = install.cert_path.clone();
    }
    if install.key_path.is_some() {
        rc.key_path = install.key_path.clone();
    }
    if install.fullchain_path.is_some() {
        rc.fullchain_path = install.fullchain_path.clone();
    }
    if install.chain_path.is_some() {
        rc.chain_path = install.chain_path.clone();
    }
    if install.reloadcmd.is_some() {
        rc.reloadcmd = install.reloadcmd.clone();
    }
    if install.deploy_hook.is_some() {
        rc.deploy_hook = install.deploy_hook.clone();
    }
    if install.pre_hook.is_some() {
        rc.pre_hook = install.pre_hook.clone();
    }
    if install.post_hook.is_some() {
        rc.post_hook = install.post_hook.clone();
    }
}

fn issue_certificate(
    config: &Config,
    rc: &RenewalConfig,
    env_overrides: &HashMap<String, String>,
    persist: bool,
    eab: Option<&acme::account::EabCredentials>,
) -> Result<(), Error> {
    if rc.domains.is_empty() {
        return Err(Error::Config("no domains to issue".into()));
    }
    if rc.email.is_empty() {
        return Err(Error::Config("email required in renewal config".into()));
    }

    let eab_required = ca::eab_required(rc.ca.as_deref(), &rc.server);
    let account =
        acme::account::load_or_register(config, &rc.email, &rc.server, eab, eab_required)?;
    let account_key = crypto::load_key_from_der(
        &base64::decode(&account.key_b64)
            .map_err(|e| Error::Config(format!("decode account key: {e}")))?,
    )?;

    let directory = acme::directory::fetch(&rc.server)?;
    let mut get_nonce = mk_nonce_fn(&directory.new_nonce);

    let nonce = get_nonce()?;
    let order = acme::order::create_order(
        &rc.domains,
        &account.url,
        &directory.new_order,
        &account_key,
        &nonce,
    )?;
    eprintln!("order created: {}", order.url);

    let auths = acme::challenge::get_authorizations(
        &account.url,
        &order.authorizations,
        &account_key,
        &mut get_nonce,
    )?;

    if rc.challenge == "http-01" {
        issue_http_challenges(&auths, &account, &account_key, &mut get_nonce)?;
    } else {
        issue_dns_challenges(
            rc,
            &auths,
            &account,
            &account_key,
            &mut get_nonce,
            env_overrides,
        )?;
    }

    eprintln!("finalizing order...");
    let cert_key = crypto::generate_p256_key()?;
    let csr = crypto::create_csr(&rc.domains, &cert_key.pkcs8_bytes)?;
    let cert_url = acme::order::finalize_order(
        &csr,
        &order.finalize,
        &account.url,
        &account_key,
        &mut get_nonce,
    )?;

    eprintln!("downloading certificate...");
    let cert_pem = acme::order::download_cert_with_preferred(
        &cert_url,
        rc.preferred_chain.as_deref(),
        &account.url,
        &account_key,
        &mut get_nonce,
    )?;

    if !persist {
        eprintln!("dry run: certificate issued successfully (not saved)");
        return Ok(());
    }

    storage::save_certificate(config, rc, &cert_pem, &cert_key.pkcs8_bytes)?;
    eprintln!(
        "certificate saved to {:?}",
        config.cert_file(rc.main_domain())
    );

    if rc.staging {
        eprintln!("staging certificate: skipping deployment");
    } else {
        deploy::deploy_certificate(config, rc)?;
    }
    Ok(())
}

fn issue_http_challenges(
    auths: &[acme::challenge::Authorization],
    account: &acme::account::Account,
    account_key: &crypto::AccountKey,
    get_nonce: &mut dyn FnMut() -> Result<String, Error>,
) -> Result<(), Error> {
    let mut challenge_pairs: Vec<(String, String)> = Vec::new();
    let mut challenge_urls: Vec<(String, String)> = Vec::new();
    for auth in auths {
        if auth.status == "valid" {
            continue;
        }
        let domain = &auth.identifier.value;
        let ch = auth
            .challenges
            .iter()
            .find(|c| c.typ == "http-01")
            .ok_or_else(|| Error::Dns(format!("no http-01 challenge for {domain}")))?;
        let token = ch.token.as_deref().unwrap_or("");
        let key_auth = ch.key_authorization(&account.jwk_thumbprint);
        challenge_pairs.push((token.to_string(), key_auth));
        challenge_urls.push((ch.url.clone(), domain.clone()));
    }

    let _server = acme::challenge::start_http_challenges(&challenge_pairs)?;

    for (url, domain) in &challenge_urls {
        eprintln!("signaling ACME server for {domain}...");
        acme::challenge::respond_to_challenge(url, &account.url, account_key, get_nonce)?;
        eprintln!("waiting for HTTP-01 validation of {domain}...");
        acme::challenge::poll_challenge(url, &account.url, account_key, get_nonce)?;
        eprintln!("{domain} validated!");
    }
    Ok(())
}

fn issue_dns_challenges(
    rc: &RenewalConfig,
    auths: &[acme::challenge::Authorization],
    account: &acme::account::Account,
    account_key: &crypto::AccountKey,
    get_nonce: &mut dyn FnMut() -> Result<String, Error>,
    env_overrides: &HashMap<String, String>,
) -> Result<(), Error> {
    let provider_meta = providers::find(&rc.provider)
        .ok_or_else(|| Error::Config(format!("unknown provider: {}", rc.provider)))?;

    let mut env = config::read_env_vars(provider_meta.env_vars);
    for (k, v) in env_overrides {
        env.insert(k.clone(), v.clone());
    }
    let provider = (provider_meta.create)(&env)?;

    struct Cleanup<'a> {
        provider: &'a dyn providers::DnsProvider,
        domain: String,
        name: String,
        value: String,
    }
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = self
                .provider
                .remove_txt(&self.domain, &self.name, &self.value);
        }
    }

    for auth in auths {
        if auth.status == "valid" {
            continue;
        }
        let domain = &auth.identifier.value;
        let dns_challenge = auth
            .challenges
            .iter()
            .find(|c| c.typ == "dns-01")
            .ok_or_else(|| Error::Dns(format!("no dns-01 challenge for {domain}")))?;
        let token = dns_challenge
            .token
            .as_deref()
            .ok_or_else(|| Error::Dns(format!("no token for {domain}")))?;

        let txt_value = acme::account::dns_txt_value(token, &account.jwk_thumbprint);
        let challenge_domain = format!("_acme-challenge.{domain}");

        eprintln!("adding TXT record {challenge_domain} = {txt_value}");
        provider.add_txt(domain, &challenge_domain, &txt_value)?;

        let _cleanup = Cleanup {
            provider: provider.as_ref(),
            domain: domain.to_string(),
            name: challenge_domain.clone(),
            value: txt_value.clone(),
        };

        wait_for_propagation(rc, &challenge_domain, &txt_value)?;

        eprintln!("signaling ACME server...");
        acme::challenge::respond_to_challenge(
            &dns_challenge.url,
            &account.url,
            account_key,
            get_nonce,
        )?;

        eprintln!("waiting for validation...");
        acme::challenge::poll_challenge(&dns_challenge.url, &account.url, account_key, get_nonce)?;

        eprintln!("{domain} validated!");
    }
    Ok(())
}

fn wait_for_propagation(rc: &RenewalConfig, name: &str, value: &str) -> Result<(), Error> {
    if rc.dns_timeout > 0 {
        let servers = dns::resolvers(&rc.dns_servers);
        if servers.is_empty() {
            eprintln!("warning: no DNS resolvers found; falling back to --dnssleep");
        } else {
            eprintln!("waiting for DNS propagation of {name}...");
            match dns::poll_txt(name, value, &servers, Duration::from_secs(rc.dns_timeout)) {
                Ok(()) => {
                    eprintln!("TXT record is visible");
                    return Ok(());
                }
                Err(e) => {
                    if rc.dnssleep == 0 {
                        return Err(Error::Dns(e));
                    }
                    eprintln!("warning: {e}; falling back to {}s sleep", rc.dnssleep);
                }
            }
        }
    }
    if rc.dnssleep > 0 {
        eprintln!("waiting {}s for DNS propagation...", rc.dnssleep);
        std::thread::sleep(Duration::from_secs(rc.dnssleep));
    }
    Ok(())
}

fn cmd_renew(args: cli::RenewArgs) -> Result<(), Error> {
    let config = Config::load()?;
    let _lock = lock::Lock::acquire(&config.home)?;
    let env_overrides = envfile::merge(
        args.env_file.as_deref().map(std::path::Path::new),
        &HashMap::new(),
    )?;
    let domains = if let Some(domain) = &args.domain {
        vec![domain.clone()]
    } else {
        config.certificate_domains()?
    };

    if domains.is_empty() {
        eprintln!("no certificates found");
        return Ok(());
    }

    let mut configs = Vec::new();
    let mut load_failures = 0usize;
    for domain in &domains {
        match RenewalConfig::load(&config, domain) {
            Ok(rc) => configs.push(rc),
            Err(e) => {
                eprintln!("warning: cannot load renewal config for {domain}: {e}");
                load_failures += 1;
            }
        }
    }

    let mut pending = Vec::new();
    for mut rc in configs {
        apply_install(&mut rc, &args.install);
        if let Some(chain) = &args.preferred_chain {
            rc.preferred_chain = Some(chain.clone());
        }

        let server = if args.test || args.dry_run {
            ca::staging().directory.to_string()
        } else if let Some(server) = &args.server {
            server.clone()
        } else if rc.server.is_empty() {
            ca::default_preset().directory.to_string()
        } else {
            rc.server.clone()
        };
        if args.test || args.dry_run || args.server.is_some() {
            rc.ca = None;
        }
        rc.server = server.clone();
        rc.staging = server == ca::staging().directory;

        let threshold = args.days.unwrap_or(30);
        let cert_path = config.cert_file(rc.main_domain());
        let days = std::fs::read_to_string(&cert_path)
            .map(|s| days_until_expiry(&s))
            .unwrap_or(0);
        let mut due = args.force || args.dry_run || !cert_path.exists() || days <= threshold;

        if !due
            && let Ok(directory) = acme::directory::fetch(&server)
            && let Ok(cert_pem) = std::fs::read_to_string(&cert_path)
            && let Some(window) = acme::ari::fetch(&directory, &cert_pem)
        {
            due = acme::ari::is_due(&window);
        }

        pending.push((rc, due, days));
    }

    let any_due = pending.iter().any(|(_, due, _)| *due);
    let pre_hook = pending.iter().find_map(|(rc, _, _)| rc.pre_hook.clone());
    let post_hook = pending.iter().find_map(|(rc, _, _)| rc.post_hook.clone());
    let hook_config = pending.first().map(|(rc, _, _)| rc.clone());

    let mut renewed = 0usize;
    let mut skipped = 0usize;
    let mut failed = load_failures;

    if any_due
        && let Some(cmd) = &pre_hook
        && let Some(first) = &hook_config
        && let Err(e) = deploy::run_hook(cmd, &config, first)
    {
        eprintln!("warning: pre-hook failed: {e}");
    }

    for (rc, due, days) in pending {
        if !due {
            eprintln!("{}: skipped ({days} days remaining)", rc.main_domain());
            skipped += 1;
            continue;
        }

        eprintln!("{}: renewing...", rc.main_domain());
        match issue_certificate(&config, &rc, &env_overrides, !args.dry_run, None) {
            Ok(()) => {
                if args.dry_run {
                    eprintln!("{}: dry run successful", rc.main_domain());
                } else {
                    eprintln!("{}: renewed", rc.main_domain());
                }
                renewed += 1;
            }
            Err(e) => {
                eprintln!("{}: failed: {e}", rc.main_domain());
                failed += 1;
            }
        }
    }

    if any_due
        && let Some(cmd) = &post_hook
        && let Some(first) = &hook_config
        && let Err(e) = deploy::run_hook(cmd, &config, first)
    {
        eprintln!("warning: post-hook failed: {e}");
    }

    eprintln!("renewal summary: {renewed} renewed, {skipped} skipped, {failed} failed");
    if failed > 0 {
        return Err(Error::Config(format!("{failed} certificate(s) failed")));
    }
    Ok(())
}

fn cmd_revoke(domain: &str, reason: Option<&str>, delete_after: bool) -> Result<(), Error> {
    let config = Config::load()?;
    let cert_pem = std::fs::read_to_string(config.cert_file(domain))?;
    let server = RenewalConfig::load(&config, domain)
        .map(|rc| rc.server)
        .unwrap_or_else(|_| config.server.clone());

    let account = acme::account::load(&config, &server)
        .ok_or_else(|| Error::Config(format!("no account for server {server}")))?;
    let account_key = crypto::load_key_from_der(
        &base64::decode(&account.key_b64).map_err(|e| Error::Crypto(format!("decode key: {e}")))?,
    )?;

    let directory = acme::directory::fetch(&server)?;
    let mut get_nonce = mk_nonce_fn(&directory.new_nonce);
    let nonce = get_nonce()?;

    let payload = serde_json::json!({
        "certificate": cert_pem,
        "reason": revoke_reason_code(reason)?,
    });

    let jws = crypto::sign_jws(
        &serde_json::to_vec(&payload).unwrap(),
        &account_key.key_pair,
        &crypto::KidOrJwk::Kid(account.url),
        &nonce,
        &directory.revoke_cert,
    )?;

    let resp = http::post(
        &directory.revoke_cert,
        &serde_json::to_vec(&jws).unwrap(),
        "application/jose+json",
        &[],
    )
    .map_err(|e| Error::Acme {
        status: 0,
        detail: e,
        error_type: "http".into(),
    })?;

    if resp.status >= 400 {
        return Err(Error::Acme {
            status: resp.status,
            detail: resp.body,
            error_type: "revoke_failed".into(),
        });
    }

    eprintln!("certificate for {domain} revoked");
    if delete_after {
        std::fs::remove_dir_all(config.domain_dir(domain))?;
        eprintln!("deleted {domain}");
    }
    Ok(())
}

fn revoke_reason_code(reason: Option<&str>) -> Result<u64, Error> {
    match reason.unwrap_or("unspecified") {
        "unspecified" => Ok(0),
        "keycompromise" => Ok(1),
        "affiliationchanged" => Ok(3),
        "superseded" => Ok(4),
        "cessationofoperation" => Ok(5),
        "privilegewithdrawn" => Ok(9),
        other => Err(Error::Config(format!(
            "unknown revocation reason: {other} (use unspecified, keycompromise, affiliationchanged, superseded, cessationofoperation, or privilegewithdrawn)"
        ))),
    }
}

fn cmd_list_providers() -> Result<(), Error> {
    println!("Available DNS providers:");
    for meta in providers::list() {
        let env = meta.env_vars.join(", ");
        println!("  {:<20} env: {env}", meta.slug);
    }
    Ok(())
}

fn cmd_register(
    email: &str,
    ca_name: Option<&str>,
    server: Option<&str>,
    test: bool,
    eab_kid: Option<&str>,
    eab_hmac_key: Option<&str>,
) -> Result<(), Error> {
    let config = Config::load()?;
    let resolved = ca::resolve(ca_name, server, test)?;
    let eab = acme::account::EabCredentials::from_options(eab_kid, eab_hmac_key)?;
    let account = acme::account::load_or_register(
        &config,
        email,
        &resolved.directory,
        eab.as_ref(),
        resolved.eab_required,
    )?;
    eprintln!("registered: {} ({})", account.url, resolved.directory);
    if let Some(name) = resolved.name {
        eprintln!("ca: {name}");
    }
    Ok(())
}

fn cmd_certificates() -> Result<(), Error> {
    let config = Config::load()?;
    let domains = config.certificate_domains()?;
    if domains.is_empty() {
        println!("no certificates found");
        return Ok(());
    }
    for domain in domains {
        let rc = RenewalConfig::load(&config, &domain).ok();
        let names = rc
            .as_ref()
            .map(|rc| rc.domains.join(", "))
            .unwrap_or_else(|| domain.clone());
        let expiry = std::fs::read_to_string(config.cert_file(&domain))
            .map(|s| days_until_expiry(&s))
            .unwrap_or(0);
        println!("{domain:<32} {expiry:>3} days  {names}");
    }
    Ok(())
}

fn cmd_show(domain: &str) -> Result<(), Error> {
    let config = Config::load()?;
    let rc = RenewalConfig::load(&config, domain)?;
    println!("name:      {domain}");
    println!("domains:   {}", rc.domains.join(", "));
    println!("provider:  {}", rc.provider);
    println!("email:     {}", rc.email);
    println!("server:    {}", rc.server);
    if let Some(ca) = &rc.ca {
        println!("ca:        {ca}");
    }
    println!("staging:   {}", rc.staging);
    println!("challenge: {}", rc.challenge);
    if let Some(chain) = &rc.preferred_chain {
        println!("chain:     {chain}");
    }
    if let Some(path) = &rc.fullchain_path {
        println!("fullchain: {path}");
    }
    if let Some(cmd) = &rc.reloadcmd {
        println!("reloadcmd: {cmd}");
    }
    let cert_path = config.cert_file(domain);
    if cert_path.exists() {
        let pem = std::fs::read_to_string(&cert_path)?;
        println!("expiry:    {} days", days_until_expiry(&pem));
    }
    Ok(())
}

fn cmd_show_account(server: Option<&str>, test: bool) -> Result<(), Error> {
    let config = Config::load()?;
    let server = ca::resolve(None, server, test)?.directory;
    let account = acme::account::load(&config, &server)
        .ok_or_else(|| Error::Config(format!("no account for server {server}")))?;
    println!("url:    {}", account.url);
    println!("server: {}", account.server);
    println!("email:  {}", account.email);
    Ok(())
}

fn cmd_delete(domain: &str) -> Result<(), Error> {
    let config = Config::load()?;
    let dir = config.domain_dir(domain);
    if !dir.exists() {
        return Err(Error::Config(format!("no certificate found for {domain}")));
    }
    std::fs::remove_dir_all(dir)?;
    eprintln!("deleted {domain}");
    Ok(())
}

fn days_until_expiry(cert_pem: &str) -> u64 {
    let der = match pem_to_der(cert_pem.as_bytes()) {
        Ok(d) => d,
        Err(_) => return 0,
    };
    use time::OffsetDateTime;
    match x509_parser::parse_x509_certificate(&der) {
        Ok((_, cert)) => {
            let not_after = cert.tbs_certificate.validity.not_after;
            let expiry = OffsetDateTime::from_unix_timestamp(not_after.timestamp())
                .unwrap_or_else(|_| OffsetDateTime::now_utc());
            let now = OffsetDateTime::now_utc();
            let secs = expiry - now;
            let days = secs.whole_days();
            if days < 0 { 0 } else { days as u64 }
        }
        Err(_) => 0,
    }
}

fn pem_to_der(pem: &[u8]) -> Result<Vec<u8>, String> {
    let s = std::str::from_utf8(pem).map_err(|_| "PEM not UTF-8".to_string())?;
    let lines: Vec<&str> = s
        .lines()
        .skip_while(|l| !l.starts_with("-----BEGIN"))
        .skip(1)
        .take_while(|l| !l.starts_with("-----END"))
        .collect();
    let b64: String = lines.join("");
    base64::decode_std(&b64).map_err(|e| format!("PEM decode: {e}"))
}
