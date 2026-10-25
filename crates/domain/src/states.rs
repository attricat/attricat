//! Stored lifecycle states. Each enum lists the values its table's `CHECK`
//! constraint allows.

crate::code_enum! {
    /// Whether an installed extension runs in its workspace.
    pub enum ExtensionState (UnknownExtensionState: "extension state") {
        Disabled => "disabled",
        Enabled => "enabled",
        /// Stopped after a failure. Re-enabling revalidates the installed
        /// release and its configuration.
        Quarantined => "quarantined",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_and_are_unique() {
        crate::codes::assert_codes_round_trip(
            ExtensionState::ALL,
            ExtensionState::as_str,
            ExtensionState::parse,
        );
    }
}
