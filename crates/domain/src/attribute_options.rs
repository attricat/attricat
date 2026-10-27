//! How an attribute declares cardinality and context behavior. Blueprints
//! and reusable attributes declare these by code, and attribute rows store
//! the code.

crate::code_enum! {
    /// How many values one side of a relationship or a file attribute holds.
    pub enum Cardinality (UnknownCardinality: "cardinality") {
        One => "one",
        Many => "many",
    }
}

crate::code_enum! {
    /// What a non-default context shows for an attribute it has no value for.
    pub enum ContextFallback (UnknownContextFallback: "context fallback") {
        /// Inherits the value of the nearest ancestor context.
        Default => "default",
        /// Leaves the attribute absent.
        None => "none",
    }
}

impl ContextFallback {
    /// Whether a missing value is inherited from an ancestor context.
    pub const fn inherits(self) -> bool {
        matches!(self, Self::Default)
    }
}

crate::code_enum! {
    /// Which contexts accept writes to an attribute.
    pub enum ContextEditable (UnknownContextEditable: "context editable scope") {
        All => "all",
        /// Only the default context.
        Default => "default",
    }
}

crate::code_enum! {
    /// When a generated attribute's numbering restarts at its `start`.
    pub enum SequenceReset (UnknownSequenceReset: "sequence reset") {
        /// One counter for the attribute's whole life.
        Never => "never",
        Yearly => "yearly",
        Monthly => "monthly",
        Daily => "daily",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_and_are_unique() {
        crate::codes::assert_codes_round_trip(
            Cardinality::ALL,
            Cardinality::as_str,
            Cardinality::parse,
        );
        crate::codes::assert_codes_round_trip(
            ContextFallback::ALL,
            ContextFallback::as_str,
            ContextFallback::parse,
        );
        crate::codes::assert_codes_round_trip(
            ContextEditable::ALL,
            ContextEditable::as_str,
            ContextEditable::parse,
        );
        crate::codes::assert_codes_round_trip(
            SequenceReset::ALL,
            SequenceReset::as_str,
            SequenceReset::parse,
        );
    }
}
