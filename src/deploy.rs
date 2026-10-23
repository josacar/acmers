use std::path::Path;

use crate::config::{self, Config};
use crate::error::Error;
use crate::renewal::RenewalConfig;

pub fn deploy_certificate(config: &Config, rc: &RenewalConfig) -> Result<(), Error> {
    let domain = rc.main_domain();
    if let Some(path) = &rc.cert_path {
        copy_artifact(&config.cert_file(domain), path, 0o644)?;
    }
    if let Some(path) = &rc.key_path {
        copy_artifact(&config.privkey_file(domain), path, 0o600)?;
    }
    if let Some(path) = &rc.fullchain_path {
        copy_artifact(&config.fullchain_file(domain), path, 0o644)?;
    }
    if let Some(path) = &rc.chain_path {
        copy_artifact(&config.chain_file(domain), path, 0o644)?;
    }
    if let Some(cmd) = &rc.reloadcmd {
        run_hook(cmd, config, rc)?;
    }
    if let Some(cmd) = &rc.deploy_hook {
        run_hook(cmd, config, rc)?;
    }
    Ok(())
}

pub fn copy_artifact(src: &Path, dest: &str, mode: u32) -> Result<(), Error> {
    let data = std::fs::read(src)?;
    config::write_atomic(Path::new(dest), &data, mode)
}

pub fn run_hook(cmd: &str, config: &Config, rc: &RenewalConfig) -> Result<(), Error> {
    let lineage = config.domain_dir(rc.main_domain());
    let status = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(cmd)
        .env("RENEWED_LINEAGE", &lineage)
        .env("RENEWED_DOMAINS", rc.domains.join(" "))
        .status()
        .map_err(|e| Error::Config(format!("run hook '{cmd}': {e}")))?;
    if !status.success() {
        return Err(Error::Config(format!("hook failed with {status}: {cmd}")));
    }
    Ok(())
}
