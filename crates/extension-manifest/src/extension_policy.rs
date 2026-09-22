//! Deployment-wide extension containment policy.
//!
//! The environment is consulted at each runtime gate rather than only at
//! startup. This lets an operator stop new extension work without changing an
//! installation, its grants, or its lifecycle history.

use std::env;

use uuid::Uuid;

/// Returns whether this deployment permits a particular extension release to
/// execute or serve client assets. `EXTENSIONS_MODE` defaults to `enabled` for
/// existing deployments but is fail-closed for invalid configured values; the
/// exact value `disabled` stops execution. `EXTENSION_DENYLIST` is a
/// comma-separated list of extension IDs or `extension-id@release-uuid` items.
pub fn allows(extension_id: &str, release_id: Uuid) -> bool {
    allows_with(
        extension_id,
        release_id,
        env::var("EXTENSIONS_MODE").ok().as_deref(),
        env::var("EXTENSION_DENYLIST").ok().as_deref(),
    )
}

fn allows_with(
    extension_id: &str,
    release_id: Uuid,
    mode: Option<&str>,
    denylist: Option<&str>,
) -> bool {
    deployment_enabled(mode) && !denylisted(extension_id, release_id, denylist)
}

fn deployment_enabled(mode: Option<&str>) -> bool {
    mode.is_none_or(|value| value == "enabled")
}

fn denylisted(extension_id: &str, release_id: Uuid, value: Option<&str>) -> bool {
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .any(|entry| entry == extension_id || entry == format!("{extension_id}@{release_id}"))
}

#[cfg(test)]
mod tests {
    use super::{allows_with, denylisted};
    use uuid::Uuid;

    #[test]
    fn deployment_controls_are_fail_closed_and_targeted() {
        let release = Uuid::new_v4();
        assert!(allows_with("acme.extension", release, None, None));
        assert!(!allows_with(
            "acme.extension",
            release,
            Some("disabled"),
            None
        ));
        assert!(!allows_with(
            "acme.extension",
            release,
            Some("invalid"),
            None
        ));
        assert!(!allows_with(
            "acme.extension",
            release,
            Some("enabled"),
            Some("acme.extension")
        ));

        assert!(allows_with(
            "acme.extension",
            release,
            Some("enabled"),
            Some("other.extension")
        ));
    }

    #[test]
    fn denylist_matches_extension_ids_and_specific_releases() {
        let release = Uuid::new_v4();
        assert!(denylisted(
            "acme.extension",
            release,
            Some("other, acme.extension")
        ));
        assert!(denylisted(
            "acme.extension",
            release,
            Some(&format!("acme.extension@{release}"))
        ));
        assert!(!denylisted(
            "acme.extension",
            release,
            Some("acme.extension@00000000-0000-0000-0000-000000000000")
        ));
    }
}
