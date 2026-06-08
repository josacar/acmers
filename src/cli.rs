use std::collections::HashMap;

#[derive(Debug)]
pub enum Cmd {
    Issue {
        domains: Vec<String>,
        provider: String,
        email: Option<String>,
        test: bool,
        standalone: bool,
        dnssleep: u64,
        env_overrides: HashMap<String, String>,
    },
    Renew {
        domain: String,
        test: bool,
    },
    Revoke {
        domain: String,
    },
    ListProviders,
    Register {
        email: String,
        test: bool,
    },
    Cron,
}

pub fn parse(args: &[String]) -> Result<Cmd, String> {
    if args.is_empty() {
        return Err("usage: acmers <command> [options]\n\ncommands:\n  issue    Issue a new certificate\n  renew    Renew a certificate\n  revoke   Revoke a certificate\n  list-providers  List supported DNS providers\n  register       Register ACME account\n  cron           Run renewal checks".into());
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
        _ => Err(format!("unknown command: {cmd_name}\nuse 'acmers' without arguments to see usage")),
    }
}

fn parse_issue(args: &[String]) -> Result<Cmd, String> {
        let mut domains = Vec::new();
        let mut provider = String::new();
        let mut email = None;
        let mut test = false;
        let mut standalone = false;
        let mut dnssleep = 10u64;
        let mut env_overrides = HashMap::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-d" | "--domain" => {
                i += 1;
                if i >= args.len() { return Err("missing domain after -d".into()); }
                domains.push(args[i].clone());
            }
            "--dns" => {
                i += 1;
                if i >= args.len() { return Err("missing provider after --dns".into()); }
                provider = args[i].clone();
            }
            "--email" | "-m" => {
                i += 1;
                if i >= args.len() { return Err("missing email after --email".into()); }
                email = Some(args[i].clone());
            }
            "--test" | "--staging" => {
                test = true;
            }
            "--dnssleep" => {
                i += 1;
                if i >= args.len() { return Err("missing seconds after --dnssleep".into()); }
                dnssleep = args[i].parse().map_err(|_| format!("invalid dnssleep: {}", args[i]))?;
            }
            "--standalone" => {
                standalone = true;
            }
            "-e" if provider.is_empty() => {
                i += 1;
                if i >= args.len() { return Err("missing var=val after -e".into()); }
                if let Some((k, v)) = args[i].split_once('=') {
                    env_overrides.insert(k.to_string(), v.to_string());
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

    Ok(Cmd::Issue { domains, provider, email, test, standalone, dnssleep, env_overrides })
}

fn parse_renew(args: &[String]) -> Result<Cmd, String> {
    let mut domain = None;
    let mut test = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-d" | "--domain" => {
                i += 1;
                if i >= args.len() { return Err("missing domain".into()); }
                domain = Some(args[i].clone());
            }
            "--test" | "--staging" => test = true,
            _ => return Err(format!("unknown option: {}", args[i])),
        }
        i += 1;
    }
    Ok(Cmd::Renew { domain: domain.ok_or("--domain required")?, test })
}

fn parse_revoke(args: &[String]) -> Result<Cmd, String> {
    let mut domain = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-d" | "--domain" => {
                i += 1;
                if i >= args.len() { return Err("missing domain".into()); }
                domain = Some(args[i].clone());
            }
            _ => return Err(format!("unknown option: {}", args[i])),
        }
        i += 1;
    }
    Ok(Cmd::Revoke { domain: domain.ok_or("--domain required")? })
}

fn parse_register(args: &[String]) -> Result<Cmd, String> {
    let mut email = None;
    let mut test = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--email" | "-m" => {
                i += 1;
                if i >= args.len() { return Err("missing email".into()); }
                email = Some(args[i].clone());
            }
            "--test" | "--staging" => test = true,
            _ => return Err(format!("unknown option: {}", args[i])),
        }
        i += 1;
    }
    Ok(Cmd::Register { email: email.ok_or("--email required")?, test })
}
