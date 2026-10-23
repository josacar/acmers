use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct InstallOptions {
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    pub fullchain_path: Option<String>,
    pub chain_path: Option<String>,
    pub reloadcmd: Option<String>,
    pub deploy_hook: Option<String>,
    pub pre_hook: Option<String>,
    pub post_hook: Option<String>,
}

#[derive(Debug)]
pub struct IssueArgs {
    pub domains: Vec<String>,
    pub provider: String,
    pub email: Option<String>,
    pub server: Option<String>,
    pub test: bool,
    pub standalone: bool,
    pub dnssleep: u64,
    pub dns_servers: Vec<String>,
    pub dns_timeout: Option<u64>,
    pub preferred_chain: Option<String>,
    pub env_file: Option<String>,
    pub env_overrides: HashMap<String, String>,
    pub install: InstallOptions,
}

#[derive(Debug)]
pub struct RenewArgs {
    pub domain: Option<String>,
    pub all: bool,
    pub server: Option<String>,
    pub test: bool,
    pub force: bool,
    pub dry_run: bool,
    pub days: Option<u64>,
    pub keep_until_expiring: bool,
    pub preferred_chain: Option<String>,
    pub env_file: Option<String>,
    pub install: InstallOptions,
}

#[derive(Debug)]
pub enum Cmd {
    Issue(IssueArgs),
    Renew(RenewArgs),
    Revoke {
        domain: String,
        reason: Option<String>,
        delete_after_revoke: bool,
    },
    ListProviders,
    Register {
        email: String,
        server: Option<String>,
        test: bool,
    },
    Cron,
    Certificates,
    Show {
        domain: String,
    },
    ShowAccount {
        server: Option<String>,
        test: bool,
    },
    Delete {
        domain: String,
    },
}

pub fn parse(args: &[String]) -> Result<Cmd, String> {
    if args.is_empty() {
        return Err(usage());
    }

    let cmd_name = &args[0];
    let rest = &args[1..];

    match cmd_name.as_str() {
        "issue" => parse_issue(rest),
        "renew" => parse_renew(rest),
        "revoke" => parse_revoke(rest),
        "list-providers" => Ok(Cmd::ListProviders),
        "register" => parse_register(rest),
        "cron" => Ok(Cmd::Cron),
        "certificates" => Ok(Cmd::Certificates),
        "show" => parse_show(rest),
        "show-account" => parse_show_account(rest),
        "delete" => parse_delete(rest),
        _ => Err(format!(
            "unknown command: {cmd_name}\nuse 'acmers' without arguments to see usage"
        )),
    }
}

fn usage() -> String {
    "usage: acmers <command> [options]\n\n\
commands:\n\
  issue           Issue a new certificate\n\
  renew           Renew one or all certificates\n\
  revoke          Revoke a certificate\n\
  list-providers  List supported DNS providers\n\
  register        Register ACME account\n\
  cron            Renew all certificates (alias for 'renew --all')\n\
  certificates    List stored certificates\n\
  show            Show certificate details\n\
  show-account    Show the active ACME account\n\
  delete          Delete a stored certificate\n"
        .to_string()
}

fn next_value(args: &[String], i: &mut usize, name: &str) -> Result<String, String> {
    *i += 1;
    if *i >= args.len() {
        return Err(format!("missing value after {name}"));
    }
    Ok(args[*i].clone())
}

fn parse_install(
    args: &[String],
    i: &mut usize,
    install: &mut InstallOptions,
) -> Result<bool, String> {
    match args[*i].as_str() {
        "--cert-path" => install.cert_path = Some(next_value(args, i, "--cert-path")?),
        "--key-path" => install.key_path = Some(next_value(args, i, "--key-path")?),
        "--fullchain-path" => {
            install.fullchain_path = Some(next_value(args, i, "--fullchain-path")?)
        }
        "--chain-path" => install.chain_path = Some(next_value(args, i, "--chain-path")?),
        "--reloadcmd" | "--reload-cmd" => {
            install.reloadcmd = Some(next_value(args, i, "--reloadcmd")?)
        }
        "--deploy-hook" => install.deploy_hook = Some(next_value(args, i, "--deploy-hook")?),
        "--pre-hook" => install.pre_hook = Some(next_value(args, i, "--pre-hook")?),
        "--post-hook" => install.post_hook = Some(next_value(args, i, "--post-hook")?),
        _ => return Ok(false),
    }
    Ok(true)
}

fn parse_issue(args: &[String]) -> Result<Cmd, String> {
    let mut domains = Vec::new();
    let mut provider = String::new();
    let mut email = None;
    let mut server = None;
    let mut test = false;
    let mut standalone = false;
    let mut dnssleep = 10u64;
    let mut dns_servers = Vec::new();
    let mut dns_timeout = None;
    let mut preferred_chain = None;
    let mut env_file = None;
    let mut env_overrides = HashMap::new();
    let mut install = InstallOptions::default();

    let mut i = 0;
    while i < args.len() {
        if parse_install(args, &mut i, &mut install)? {
            i += 1;
            continue;
        }
        match args[i].as_str() {
            "-d" | "--domain" => domains.push(next_value(args, &mut i, "-d")?),
            "--dns" => provider = next_value(args, &mut i, "--dns")?,
            "--email" | "-m" => email = Some(next_value(args, &mut i, "--email")?),
            "--server" => server = Some(next_value(args, &mut i, "--server")?),
            "--test" | "--staging" => test = true,
            "--dnssleep" => {
                dnssleep = next_value(args, &mut i, "--dnssleep")?
                    .parse()
                    .map_err(|_| "invalid dnssleep".to_string())?
            }
            "--dns-timeout" => {
                dns_timeout = Some(
                    next_value(args, &mut i, "--dns-timeout")?
                        .parse()
                        .map_err(|_| "invalid dns-timeout".to_string())?,
                )
            }
            "--dns-servers" => dns_servers.extend(
                next_value(args, &mut i, "--dns-servers")?
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
            ),
            "--preferred-chain" => {
                preferred_chain = Some(next_value(args, &mut i, "--preferred-chain")?)
            }
            "--env-file" => env_file = Some(next_value(args, &mut i, "--env-file")?),
            "--standalone" => standalone = true,
            "-e" => {
                let pair = next_value(args, &mut i, "-e")?;
                if let Some((k, v)) = pair.split_once('=') {
                    env_overrides.insert(k.to_string(), v.to_string());
                } else {
                    return Err(format!("invalid -e value (expected KEY=VALUE): {pair}"));
                }
            }
            _ => return Err(format!("unknown option: {}", args[i])),
        }
        i += 1;
    }

    if domains.is_empty() {
        return Err("at least one -d domain required".into());
    }
    if !standalone && provider.is_empty() {
        return Err("--dns provider required (or use --standalone)".into());
    }

    Ok(Cmd::Issue(IssueArgs {
        domains,
        provider,
        email,
        server,
        test,
        standalone,
        dnssleep,
        dns_servers,
        dns_timeout,
        preferred_chain,
        env_file,
        env_overrides,
        install,
    }))
}

fn parse_renew(args: &[String]) -> Result<Cmd, String> {
    let mut domain = None;
    let mut all = false;
    let mut server = None;
    let mut test = false;
    let mut force = false;
    let mut dry_run = false;
    let mut days = None;
    let mut keep_until_expiring = false;
    let mut preferred_chain = None;
    let mut env_file = None;
    let mut install = InstallOptions::default();

    let mut i = 0;
    while i < args.len() {
        if parse_install(args, &mut i, &mut install)? {
            i += 1;
            continue;
        }
        match args[i].as_str() {
            "-d" | "--domain" => domain = Some(next_value(args, &mut i, "-d")?),
            "--all" => all = true,
            "--server" => server = Some(next_value(args, &mut i, "--server")?),
            "--test" | "--staging" => test = true,
            "--force" => force = true,
            "--dry-run" => dry_run = true,
            "--keep-until-expiring" => keep_until_expiring = true,
            "--preferred-chain" => {
                preferred_chain = Some(next_value(args, &mut i, "--preferred-chain")?)
            }
            "--env-file" => env_file = Some(next_value(args, &mut i, "--env-file")?),
            "--days" => {
                days = Some(
                    next_value(args, &mut i, "--days")?
                        .parse()
                        .map_err(|_| "invalid days".to_string())?,
                )
            }
            _ => return Err(format!("unknown option: {}", args[i])),
        }
        i += 1;
    }

    Ok(Cmd::Renew(RenewArgs {
        domain,
        all,
        server,
        test,
        force,
        dry_run,
        days,
        keep_until_expiring,
        preferred_chain,
        env_file,
        install,
    }))
}

fn parse_revoke(args: &[String]) -> Result<Cmd, String> {
    let mut domain = None;
    let mut reason = None;
    let mut delete_after_revoke = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-d" | "--domain" => domain = Some(next_value(args, &mut i, "-d")?),
            "--reason" => reason = Some(next_value(args, &mut i, "--reason")?),
            "--delete-after-revoke" => delete_after_revoke = true,
            _ => return Err(format!("unknown option: {}", args[i])),
        }
        i += 1;
    }
    Ok(Cmd::Revoke {
        domain: domain.ok_or("--domain required")?,
        reason,
        delete_after_revoke,
    })
}

fn parse_register(args: &[String]) -> Result<Cmd, String> {
    let mut email = None;
    let mut server = None;
    let mut test = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--email" | "-m" => email = Some(next_value(args, &mut i, "--email")?),
            "--server" => server = Some(next_value(args, &mut i, "--server")?),
            "--test" | "--staging" => test = true,
            _ => return Err(format!("unknown option: {}", args[i])),
        }
        i += 1;
    }
    Ok(Cmd::Register {
        email: email.ok_or("--email required")?,
        server,
        test,
    })
}

fn parse_show(args: &[String]) -> Result<Cmd, String> {
    let mut domain = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-d" | "--domain" => domain = Some(next_value(args, &mut i, "-d")?),
            _ => return Err(format!("unknown option: {}", args[i])),
        }
        i += 1;
    }
    Ok(Cmd::Show {
        domain: domain.ok_or("--domain required")?,
    })
}

fn parse_show_account(args: &[String]) -> Result<Cmd, String> {
    let mut server = None;
    let mut test = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--server" => server = Some(next_value(args, &mut i, "--server")?),
            "--test" | "--staging" => test = true,
            _ => return Err(format!("unknown option: {}", args[i])),
        }
        i += 1;
    }
    Ok(Cmd::ShowAccount { server, test })
}

fn parse_delete(args: &[String]) -> Result<Cmd, String> {
    let mut domain = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-d" | "--domain" => domain = Some(next_value(args, &mut i, "-d")?),
            _ => return Err(format!("unknown option: {}", args[i])),
        }
        i += 1;
    }
    Ok(Cmd::Delete {
        domain: domain.ok_or("--domain required")?,
    })
}
