use crate::acme::order;
use crate::config::{self, Config};
use crate::crypto;
use crate::error::Error;
use crate::renewal::RenewalConfig;

pub fn save_certificate(
    config: &Config,
    rc: &RenewalConfig,
    cert_pem: &str,
    key_pkcs8: &[u8],
) -> Result<(), Error> {
    let domain = rc.main_domain();
    let chain = order::split_cert_chain(cert_pem);
    let key_pem = crypto::pkcs8_to_pem(key_pkcs8);
    let dir = config.domain_dir(domain);
    std::fs::create_dir_all(&dir)?;
    config::set_mode(&dir, 0o755)?;

    config::write_atomic(&config.cert_file(domain), chain.leaf.as_bytes(), 0o644)?;
    config::write_atomic(&config.chain_file(domain), chain.chain.as_bytes(), 0o644)?;
    config::write_atomic(
        &config.fullchain_file(domain),
        chain.fullchain.as_bytes(),
        0o644,
    )?;
    config::write_atomic(&config.privkey_file(domain), key_pem.as_bytes(), 0o600)?;
    config::write_atomic(&config.key_file(domain), key_pem.as_bytes(), 0o600)?;
    rc.save(config)?;
    Ok(())
}
