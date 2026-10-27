//! Record system tags that Core owns. A Core tag starts with
//! [`CORE_SYSTEM_TAG_PREFIX`] and has no `:`, so it cannot collide with an
//! extension's `<extension-id>:<tag>`. Callers may search and sort by Core
//! tags but never add or remove them.

/// Prefix reserved for Core tags.
pub const CORE_SYSTEM_TAG_PREFIX: &str = "attricat.";
/// Marks records created as solution-pack sample data. Only the host adds it;
/// ordinary tag writes may drop it, and record copies do.
pub const SAMPLE_SYSTEM_TAG: &str = "attricat.sample";
/// Marks records with at least one open rule finding. Rule evaluation and
/// finding acknowledgement keep it current.
pub const NEEDS_ATTENTION_SYSTEM_TAG: &str = "attricat.needs_attention";

/// Every Core tag the host sets. A record carries at most these on top of its
/// ordinary tags.
pub const CORE_SYSTEM_TAGS: [&str; 2] = [SAMPLE_SYSTEM_TAG, NEEDS_ATTENTION_SYSTEM_TAG];

/// Whether `tag` is reserved for Core.
pub fn is_core_system_tag(tag: &str) -> bool {
    tag.starts_with(CORE_SYSTEM_TAG_PREFIX) && !tag.contains(':')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_tags_exclude_extension_namespaces() {
        assert!(is_core_system_tag(SAMPLE_SYSTEM_TAG));
        assert!(is_core_system_tag(NEEDS_ATTENTION_SYSTEM_TAG));
        assert!(is_core_system_tag("attricat.future"));
        assert!(!is_core_system_tag("reviewed"));
        assert!(!is_core_system_tag("attricat.example:reviewed"));
        assert!(CORE_SYSTEM_TAGS.iter().all(|tag| is_core_system_tag(tag)));
    }
}
