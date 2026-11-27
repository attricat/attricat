//! Deployment settings the web app reads before it renders, without a session.

use std::str::FromStr;

use axum::{Json, extract::State};

use super::AppState;

/// A language the web app is translated into.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InterfaceLanguage {
    En,
    Pl,
}

impl InterfaceLanguage {
    pub const ALL: [Self; 2] = [Self::En, Self::Pl];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Pl => "pl",
        }
    }
}

impl FromStr for InterfaceLanguage {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|language| language.as_str() == value)
            .ok_or(())
    }
}

/// Published to anyone who can reach the server: it holds nothing the
/// sign-in page would not show anyway.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct ClientConfig {
    /// `ATTRICAT_DEFAULT_LANGUAGE`: shown unless the person picked another.
    /// `null` leaves the choice to the browser language.
    pub default_language: Option<InterfaceLanguage>,
    /// Set by `ATTRICAT_SINGLE_WORKSPACE`: the sign-in identifier of the only
    /// workspace, which sign-in opens without asking for one.
    pub sign_in_workspace: Option<String>,
}

pub(super) async fn client_config(State(state): State<AppState>) -> Json<ClientConfig> {
    Json(state.client_config)
}

#[cfg(test)]
mod tests {
    use super::InterfaceLanguage;

    #[test]
    fn languages_parse_from_their_codes_only() {
        for language in InterfaceLanguage::ALL {
            assert_eq!(language.as_str().parse(), Ok(language));
        }
        assert!("PL".parse::<InterfaceLanguage>().is_err());
        assert!("de".parse::<InterfaceLanguage>().is_err());
    }
}
