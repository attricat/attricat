//! Attribute value types: what an attribute stores. Blueprints declare them
//! by code, and attributes and stored values carry the code.

crate::code_enum! {
    /// The value type an attribute declares.
    pub enum ValueType (UnknownValueType: "value type") {
        String => "string",
        Number => "number",
        Integer => "integer",
        Boolean => "boolean",
        Date => "date",
        Datetime => "datetime",
        Time => "time",
        Json => "json",
        /// Stored as record links, not an attribute value.
        Relationship => "relationship",
        /// File metadata is stored in attribute_file_references, not a native
        /// attribute_values value column.
        File => "file",
    }
}

impl ValueType {
    /// A single comparable value: every type but `json`, `relationship` and
    /// `file`. Scalars sort, take presence filters and render in table cells.
    pub const fn is_scalar(self) -> bool {
        match self {
            Self::String
            | Self::Number
            | Self::Integer
            | Self::Boolean
            | Self::Date
            | Self::Datetime
            | Self::Time => true,
            Self::Json | Self::Relationship | Self::File => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_and_are_unique() {
        crate::codes::assert_codes_round_trip(ValueType::ALL, ValueType::as_str, ValueType::parse);
    }
}
