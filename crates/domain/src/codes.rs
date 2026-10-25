//! Closed sets of stored codes. [`code_enum!`](crate::code_enum) declares an
//! enum whose variants map one-to-one to the codes stored and sent over the
//! wire, so every use refers to a variant instead of a string literal.

#[doc(hidden)]
pub use serde as __serde;

/// Declares a `Copy` enum of codes with `ALL`, `as_str`, `parse`,
/// `TryFrom<String>` (for `#[sqlx(try_from = "String")]`), `FromStr`,
/// `Display`, and serde as the code string, plus an error type naming the
/// unknown code.
#[macro_export]
macro_rules! code_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident ($error:ident: $what:literal) {
            $($(#[$variant_meta:meta])* $variant:ident => $code:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        $vis enum $name {
            $($(#[$variant_meta])* $variant),+
        }

        impl $name {
            /// Every variant, in declaration order.
            pub const ALL: &'static [$name] = &[$(Self::$variant),+];

            /// Every code, in declaration order, as JSON Schema `enum` lists
            /// take them.
            pub fn codes() -> Vec<&'static str> {
                vec![$($code),+]
            }

            /// The stored code.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $code),+
                }
            }

            pub fn parse(code: &str) -> Option<Self> {
                match code {
                    $($code => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        $vis struct $error(pub String);

        impl ::std::fmt::Display for $error {
            fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(formatter, concat!("unknown ", $what, " '{}'"), self.0)
            }
        }

        impl ::std::error::Error for $error {}

        impl ::std::convert::TryFrom<String> for $name {
            type Error = $error;

            fn try_from(code: String) -> Result<Self, Self::Error> {
                Self::parse(&code).ok_or($error(code))
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = $error;

            fn from_str(code: &str) -> Result<Self, Self::Err> {
                Self::parse(code).ok_or_else(|| $error(code.to_owned()))
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl $crate::codes::__serde::Serialize for $name {
            fn serialize<S: $crate::codes::__serde::Serializer>(
                &self,
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> $crate::codes::__serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::codes::__serde::Deserializer<'de>>(
                deserializer: D,
            ) -> Result<Self, D::Error> {
                let code = <String as $crate::codes::__serde::Deserialize>::deserialize(deserializer)?;
                Self::parse(&code)
                    .ok_or_else(|| <D::Error as $crate::codes::__serde::de::Error>::custom($error(code)))
            }
        }
    };
}

#[cfg(test)]
pub(crate) fn assert_codes_round_trip<T>(
    all: &[T],
    as_str: fn(T) -> &'static str,
    parse: fn(&str) -> Option<T>,
) where
    T: Copy + PartialEq + std::fmt::Debug,
{
    let mut codes = std::collections::HashSet::new();
    for &variant in all {
        assert!(
            codes.insert(as_str(variant)),
            "duplicate code {}",
            as_str(variant)
        );
        assert_eq!(parse(as_str(variant)), Some(variant));
    }
}
