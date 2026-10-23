use crate::error::Error;

pub struct CaPreset {
    pub name: &'static str,
    pub directory: &'static str,
    pub eab_required: bool,
}

pub const PRESETS: &[CaPreset] = &[
    CaPreset {
        name: "letsencrypt",
        directory: "https://acme-v02.api.letsencrypt.org/directory",
        eab_required: false,
    },
    CaPreset {
        name: "letsencrypt-staging",
        directory: "https://acme-staging-v02.api.letsencrypt.org/directory",
        eab_required: false,
    },
    CaPreset {
        name: "zerossl",
        directory: "https://acme.zerossl.com/v2/DV90",
        eab_required: true,
    },
    CaPreset {
        name: "google",
        directory: "https://dv.acme-v02.api.pki.goog/directory",
        eab_required: true,
    },
    CaPreset {
        name: "sslcom",
        directory: "https://acme.ssl.com/sslcom-dv-ecc",
        eab_required: true,
    },
    CaPreset {
        name: "sslcom-rsa",
        directory: "https://acme.ssl.com/sslcom-dv-rsa",
        eab_required: true,
    },
    CaPreset {
        name: "buypass",
        directory: "https://api.buypass.com/acme/directory",
        eab_required: false,
    },
    CaPreset {
        name: "buypass-test",
        directory: "https://api.test4.buypass.no/acme/directory",
        eab_required: false,
    },
    CaPreset {
        name: "sectigo",
        directory: "https://acme.sectigo.com/v2/DV",
        eab_required: true,
    },
];

pub fn find(name: &str) -> Option<&'static CaPreset> {
    PRESETS.iter().find(|p| p.name == name)
}

pub fn default_preset() -> &'static CaPreset {
    &PRESETS[0]
}

pub fn staging() -> &'static CaPreset {
    find("letsencrypt-staging").unwrap_or_else(default_preset)
}

pub fn supported_names() -> String {
    PRESETS
        .iter()
        .map(|p| p.name)
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn find_by_directory(directory: &str) -> Option<&'static CaPreset> {
    PRESETS
        .iter()
        .find(|p| p.directory.trim_end_matches('/') == directory.trim_end_matches('/'))
}

pub fn eab_required(ca: Option<&str>, server: &str) -> bool {
    if let Some(preset) = ca.and_then(find) {
        return preset.eab_required;
    }
    find_by_directory(server)
        .map(|p| p.eab_required)
        .unwrap_or(false)
}

fn is_letsencrypt(preset: &CaPreset) -> bool {
    preset.name == "letsencrypt" || preset.name == "letsencrypt-staging"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCa {
    pub name: Option<&'static str>,
    pub directory: String,
    pub eab_required: bool,
}

pub fn resolve(ca: Option<&str>, server: Option<&str>, test: bool) -> Result<ResolvedCa, Error> {
    let explicit = match ca {
        Some(name) => Some(find(name).ok_or_else(|| {
            Error::Config(format!(
                "unknown CA preset: {name} (supported: {})",
                supported_names()
            ))
        })?),
        None => None,
    };

    let inferred = explicit.or_else(|| server.and_then(find_by_directory));
    let preset = inferred.unwrap_or_else(default_preset);

    if test && !is_letsencrypt(preset) {
        return Err(Error::Config(format!(
            "--test conflicts with CA {}; use --ca letsencrypt-staging instead",
            preset.name
        )));
    }

    let effective = if test { staging() } else { preset };

    let directory = if test {
        effective.directory.to_string()
    } else {
        server.unwrap_or(effective.directory).to_string()
    };

    Ok(ResolvedCa {
        name: explicit.map(|_| effective.name),
        directory,
        eab_required: effective.eab_required,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_default_to_letsencrypt_production() {
        let resolved = resolve(None, None, false).unwrap();
        assert_eq!(
            resolved.directory,
            "https://acme-v02.api.letsencrypt.org/directory"
        );
        assert_eq!(resolved.name, None);
        assert!(!resolved.eab_required);
    }

    #[test]
    fn resolves_every_preset_to_its_directory() {
        for preset in PRESETS {
            let resolved = resolve(Some(preset.name), None, false).unwrap();
            assert_eq!(resolved.directory, preset.directory);
            assert_eq!(resolved.name, Some(preset.name));
            assert_eq!(resolved.eab_required, preset.eab_required);
        }
    }

    #[test]
    fn zerossl_requires_eab() {
        let resolved = resolve(Some("zerossl"), None, false).unwrap();
        assert_eq!(resolved.directory, "https://acme.zerossl.com/v2/DV90");
        assert!(resolved.eab_required);
    }

    #[test]
    fn sslcom_rsa_uses_rsa_directory() {
        let resolved = resolve(Some("sslcom-rsa"), None, false).unwrap();
        assert_eq!(resolved.directory, "https://acme.ssl.com/sslcom-dv-rsa");
    }

    #[test]
    fn unknown_preset_lists_supported_names() {
        let err = resolve(Some("nope"), None, false).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("unknown CA preset: nope"));
        assert!(message.contains("zerossl"));
        assert!(message.contains("sectigo"));
    }

    #[test]
    fn server_override_keeps_preset_eab_requirement() {
        let resolved = resolve(
            Some("zerossl"),
            Some("https://acme.example/directory"),
            false,
        )
        .unwrap();
        assert_eq!(resolved.directory, "https://acme.example/directory");
        assert_eq!(resolved.name, Some("zerossl"));
        assert!(resolved.eab_required);
    }

    #[test]
    fn server_override_without_preset_has_no_eab_requirement() {
        let resolved = resolve(None, Some("https://acme.example/directory"), false).unwrap();
        assert_eq!(resolved.directory, "https://acme.example/directory");
        assert_eq!(resolved.name, None);
        assert!(!resolved.eab_required);
    }

    #[test]
    fn known_server_infers_eab_requirement() {
        let resolved = resolve(None, Some("https://acme.sectigo.com/v2/DV"), false).unwrap();
        assert_eq!(resolved.directory, "https://acme.sectigo.com/v2/DV");
        assert_eq!(resolved.name, None);
        assert!(resolved.eab_required);
    }

    #[test]
    fn test_selects_letsencrypt_staging() {
        let resolved = resolve(None, None, true).unwrap();
        assert_eq!(
            resolved.directory,
            "https://acme-staging-v02.api.letsencrypt.org/directory"
        );
        assert!(!resolved.eab_required);
    }

    #[test]
    fn test_with_letsencrypt_preset_stores_staging_name() {
        let resolved = resolve(Some("letsencrypt"), None, true).unwrap();
        assert_eq!(
            resolved.directory,
            "https://acme-staging-v02.api.letsencrypt.org/directory"
        );
        assert_eq!(resolved.name, Some("letsencrypt-staging"));
    }

    #[test]
    fn test_conflicts_with_non_letsencrypt_preset() {
        let err = resolve(Some("zerossl"), None, true).unwrap_err();
        assert!(err.to_string().contains("--test conflicts with CA zerossl"));
        assert!(resolve(Some("buypass"), None, true).is_err());
    }

    #[test]
    fn eab_requirement_can_be_derived_from_server() {
        assert!(eab_required(None, "https://acme.sectigo.com/v2/DV"));
        assert!(!eab_required(
            None,
            "https://acme-v02.api.letsencrypt.org/directory"
        ));
        assert!(eab_required(Some("google"), "https://custom.example/dir"));
        assert!(!eab_required(Some("buypass"), "https://custom.example/dir"));
    }

    #[test]
    fn lists_supported_names() {
        let names = supported_names();
        for preset in PRESETS {
            assert!(names.contains(preset.name));
        }
    }
}
