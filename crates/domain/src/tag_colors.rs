//! Colors a workspace gives record system tags. Clients render each code
//! with their own light and dark mode variant, so only the code is stored.

crate::code_enum! {
    /// A preset tag color.
    pub enum TagColor (UnknownTagColor: "tag color") {
        Gray => "gray",
        Blue => "blue",
        Teal => "teal",
        Green => "green",
        Yellow => "yellow",
        Orange => "orange",
        Red => "red",
        Purple => "purple",
        Pink => "pink",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_and_are_unique() {
        crate::codes::assert_codes_round_trip(TagColor::ALL, TagColor::as_str, TagColor::parse);
        assert_eq!(TagColor::parse("grey"), None);
    }
}
