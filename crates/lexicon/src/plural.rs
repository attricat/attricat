/// Every CLDR cardinal plural category, in CLDR order.
pub const PLURAL_CATEGORIES: &[&str] = &["zero", "one", "two", "few", "many", "other"];

/// The category used by entries without count-dependent forms.
pub const DEFAULT_PLURAL_CATEGORY: &str = "other";

const ONE_OTHER: &[&str] = &["one", "other"];
const ONE_MANY_OTHER: &[&str] = &["one", "many", "other"];
const ONE_FEW_MANY_OTHER: &[&str] = &["one", "few", "many", "other"];
const ONE_FEW_OTHER: &[&str] = &["one", "few", "other"];
const OTHER: &[&str] = &["other"];

/// CLDR 46 cardinal plural categories by primary language subtag. Languages
/// outside this table are not accepted, so every stored plural form can be
/// checked against its language.
const LANGUAGES: &[(&str, &[&str])] = &[
    ("ar", &["zero", "one", "two", "few", "many", "other"]),
    ("bg", ONE_OTHER),
    ("ca", ONE_MANY_OTHER),
    ("cs", ONE_FEW_MANY_OTHER),
    ("cy", &["zero", "one", "two", "few", "many", "other"]),
    ("da", ONE_OTHER),
    ("de", ONE_OTHER),
    ("el", ONE_OTHER),
    ("en", ONE_OTHER),
    ("es", ONE_MANY_OTHER),
    ("et", ONE_OTHER),
    ("fi", ONE_OTHER),
    ("fr", ONE_MANY_OTHER),
    ("ga", &["one", "two", "few", "many", "other"]),
    ("he", &["one", "two", "other"]),
    ("hr", ONE_FEW_OTHER),
    ("hu", ONE_OTHER),
    ("id", OTHER),
    ("is", ONE_OTHER),
    ("it", ONE_MANY_OTHER),
    ("ja", OTHER),
    ("ko", OTHER),
    ("lt", ONE_FEW_MANY_OTHER),
    ("lv", &["zero", "one", "other"]),
    ("nb", ONE_OTHER),
    ("nl", ONE_OTHER),
    ("nn", ONE_OTHER),
    ("no", ONE_OTHER),
    ("pl", ONE_FEW_MANY_OTHER),
    ("pt", ONE_MANY_OTHER),
    ("ro", ONE_FEW_OTHER),
    ("ru", ONE_FEW_MANY_OTHER),
    ("sk", ONE_FEW_MANY_OTHER),
    ("sl", &["one", "two", "few", "other"]),
    ("sr", ONE_FEW_OTHER),
    ("sv", ONE_OTHER),
    ("th", OTHER),
    ("tr", ONE_OTHER),
    ("uk", ONE_FEW_MANY_OTHER),
    ("vi", OTHER),
    ("zh", OTHER),
];

/// Canonicalizes a BCP 47 tag such as `pt-br` to `pt-BR`, or returns `None`
/// for malformed tags and unsupported primary languages.
pub fn canonical_language(language: &str) -> Option<String> {
    let mut subtags = language.split('-');
    let primary = subtags.next()?.to_ascii_lowercase();
    if !(2..=3).contains(&primary.len())
        || !primary.bytes().all(|byte| byte.is_ascii_lowercase())
        || plural_categories(&primary).is_none()
    {
        return None;
    }
    let mut canonical = primary;
    for subtag in subtags {
        if !(2..=8).contains(&subtag.len()) || !subtag.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return None;
        }
        canonical.push('-');
        match subtag.len() {
            2 => canonical.push_str(&subtag.to_ascii_uppercase()),
            4 => {
                let (first, rest) = subtag.split_at(1);
                canonical.push_str(&first.to_ascii_uppercase());
                canonical.push_str(&rest.to_ascii_lowercase());
            }
            _ => canonical.push_str(&subtag.to_ascii_lowercase()),
        }
    }
    Some(canonical)
}

/// The cardinal plural categories CLDR defines for `language`'s primary
/// subtag, always including `other`.
pub fn plural_categories(language: &str) -> Option<&'static [&'static str]> {
    let primary = language.split('-').next()?.to_ascii_lowercase();
    LANGUAGES
        .iter()
        .find(|(code, _)| *code == primary)
        .map(|(_, categories)| *categories)
}

/// Supported primary language subtags, for error messages and docs.
pub fn supported_languages() -> impl Iterator<Item = &'static str> {
    LANGUAGES.iter().map(|(code, _)| *code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_supported_tags() {
        assert_eq!(canonical_language("PL").as_deref(), Some("pl"));
        assert_eq!(canonical_language("pt-br").as_deref(), Some("pt-BR"));
        assert_eq!(
            canonical_language("zh-hant-tw").as_deref(),
            Some("zh-Hant-TW")
        );
        assert_eq!(canonical_language("xx"), None);
        assert_eq!(canonical_language("en_US"), None);
        assert_eq!(canonical_language("en-"), None);
        assert_eq!(plural_categories("pl-PL"), Some(ONE_FEW_MANY_OTHER));
    }

    #[test]
    fn every_language_has_ordered_categories_ending_in_other() {
        for (_, categories) in LANGUAGES {
            assert_eq!(categories.last(), Some(&DEFAULT_PLURAL_CATEGORY));
            let positions: Vec<_> = categories
                .iter()
                .map(|category| PLURAL_CATEGORIES.iter().position(|c| c == category))
                .collect();
            assert!(positions.iter().all(Option::is_some));
            assert!(positions.is_sorted());
        }
    }
}
