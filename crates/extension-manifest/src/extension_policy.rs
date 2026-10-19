//! Deployment-wide extension containment policy.
//!
//! The API parses the policy with the rest of its configuration and installs
//! it once at startup; every runtime gate then consults the installed policy.
//! It stops new extension work without changing an installation, its grants,
//! or its lifecycle history.

use std::sync::OnceLock;

use uuid::Uuid;

static INSTALLED: OnceLock<ExtensionPolicy> = OnceLock::new();

/// `EXTENSIONS_MODE` (`enabled` by default, or `disabled` to stop all
/// execution) and `EXTENSION_DENYLIST`, a comma-separated list of extension
/// IDs or `extension-id@release-uuid` items.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExtensionPolicy {
    disabled: bool,
    denylist: Vec<String>,
}

impl ExtensionPolicy {
    pub fn parse(mode: Option<&str>, denylist: Option<&str>) -> Result<Self, String> {
        let disabled = match mode {
            None | Some("enabled") => false,
            Some("disabled") => true,
            Some(_) => return Err("EXTENSIONS_MODE must be `enabled` or `disabled`".to_owned()),
        };
        let denylist = denylist
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_owned)
            .collect();
        Ok(Self { disabled, denylist })
    }

    pub fn allows(&self, extension_id: &str, release_id: Uuid) -> bool {
        !self.disabled && !self.denylisted(extension_id, release_id)
    }

    fn denylisted(&self, extension_id: &str, release_id: Uuid) -> bool {
        let release = format!("{extension_id}@{release_id}");
        self.denylist
            .iter()
            .any(|entry| entry == extension_id || *entry == release)
    }
}

/// Installs this process's policy. Call once, before serving requests or
/// starting workers.
pub fn install(policy: ExtensionPolicy) {
    INSTALLED
        .set(policy)
        .expect("the extension policy is installed once");
}

/// Returns whether this deployment permits a particular extension release to
/// execute or serve client assets. A process that never installed a policy,
/// such as a test harness, allows every extension.
pub fn allows(extension_id: &str, release_id: Uuid) -> bool {
    INSTALLED
        .get()
        .is_none_or(|policy| policy.allows(extension_id, release_id))
}

#[cfg(test)]
mod tests {
    use super::ExtensionPolicy;
    use uuid::Uuid;

    #[test]
    fn deployment_controls_are_strict_and_targeted() {
        let release = Uuid::new_v4();
        let allows = |mode, denylist| {
            ExtensionPolicy::parse(mode, denylist)
                .unwrap()
                .allows("acme.extension", release)
        };
        assert!(allows(None, None));
        assert!(!allows(Some("disabled"), None));
        assert!(ExtensionPolicy::parse(Some("invalid"), None).is_err());
        assert!(!allows(Some("enabled"), Some("acme.extension")));
        assert!(allows(Some("enabled"), Some("other.extension")));
    }

    #[test]
    fn denylist_matches_extension_ids_and_specific_releases() {
        let release = Uuid::new_v4();
        let denied = |denylist: &str| {
            !ExtensionPolicy::parse(None, Some(denylist))
                .unwrap()
                .allows("acme.extension", release)
        };
        assert!(denied("other, acme.extension"));
        assert!(denied(&format!("acme.extension@{release}")));
        assert!(!denied(
            "acme.extension@00000000-0000-0000-0000-000000000000"
        ));
    }
}
