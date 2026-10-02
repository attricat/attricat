use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::{Entry, Reference, plural::plural_categories};

/// The language whose lexicon text falls back to the reference key.
pub const SOURCE_LANGUAGE: &str = "en";

/// A reference found in catalog text. `counted` references are rendered with
/// a count by the app (for example blueprint names in result totals) and
/// therefore need every plural form of each language.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UsedReference {
    pub reference: Reference,
    pub counted: bool,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Report {
    /// Distinct references used by catalog text.
    pub reference_count: usize,
    pub languages: Vec<LanguageReport>,
    /// Entries whose key and context no catalog text references.
    pub orphaned: Vec<OrphanedEntry>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct LanguageReport {
    pub language: String,
    /// References with at least one entry in this language.
    pub translated_count: usize,
    /// References without any entry in this language. Always empty for `en`,
    /// whose text falls back to the key.
    pub untranslated: Vec<Reference>,
    pub missing_plural_categories: Vec<MissingPluralCategories>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct MissingPluralCategories {
    pub key: String,
    pub context: Option<String>,
    pub missing: Vec<&'static str>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct OrphanedEntry {
    pub key: String,
    pub context: Option<String>,
    pub languages: Vec<String>,
}

type Identity = (String, Option<String>);

/// Reports untranslated references, incomplete plural forms, and orphaned
/// entries. `languages` selects the reported languages; when empty, `en` and
/// every language with entries are reported.
pub fn report(references: &[UsedReference], entries: &[Entry], languages: &[String]) -> Report {
    let mut used: BTreeMap<Identity, bool> = BTreeMap::new();
    for used_reference in references {
        let counted = used
            .entry((
                used_reference.reference.key.clone(),
                used_reference.reference.context.clone(),
            ))
            .or_default();
        *counted |= used_reference.counted;
    }
    // language -> identity -> present plural categories
    let mut present: BTreeMap<&str, BTreeMap<Identity, BTreeSet<&str>>> = BTreeMap::new();
    let mut orphaned: BTreeMap<Identity, BTreeSet<String>> = BTreeMap::new();
    for entry in entries {
        let identity = (entry.key.clone(), entry.context.clone());
        if !used.contains_key(&identity) {
            orphaned
                .entry(identity.clone())
                .or_default()
                .insert(entry.language.clone());
        }
        present
            .entry(&entry.language)
            .or_default()
            .entry(identity)
            .or_default()
            .insert(&entry.plural_category);
    }
    let languages: BTreeSet<String> = if languages.is_empty() {
        present
            .keys()
            .map(|language| (*language).to_owned())
            .chain([SOURCE_LANGUAGE.to_owned()])
            .collect()
    } else {
        languages.iter().cloned().collect()
    };
    let empty = BTreeMap::new();
    Report {
        reference_count: used.len(),
        languages: languages
            .into_iter()
            .map(|language| {
                let source = language == SOURCE_LANGUAGE;
                let present = present.get(language.as_str()).unwrap_or(&empty);
                let required = plural_categories(&language).unwrap_or_default();
                let untranslated = used
                    .keys()
                    .filter(|identity| !source && !present.contains_key(*identity))
                    .map(|(key, context)| Reference {
                        key: key.clone(),
                        context: context.clone(),
                    })
                    .collect();
                let identities: BTreeSet<&Identity> = used.keys().chain(present.keys()).collect();
                let missing_plural_categories = identities
                    .into_iter()
                    .filter_map(|identity| {
                        let categories = present.get(identity);
                        let counted = used.get(identity).copied().unwrap_or_default();
                        let has_plural_forms = categories.is_some_and(|categories| {
                            categories.iter().any(|category| *category != "other")
                        });
                        // Untranslated references are already listed; English
                        // has no untranslated list, so its gaps appear here.
                        if !(counted || has_plural_forms) || (categories.is_none() && !source) {
                            return None;
                        }
                        let missing: Vec<_> = required
                            .iter()
                            .copied()
                            .filter(|category| {
                                categories.is_none_or(|categories| !categories.contains(category))
                            })
                            .collect();
                        (!missing.is_empty()).then(|| MissingPluralCategories {
                            key: identity.0.clone(),
                            context: identity.1.clone(),
                            missing,
                        })
                    })
                    .collect();
                LanguageReport {
                    translated_count: used
                        .keys()
                        .filter(|identity| present.contains_key(*identity))
                        .count(),
                    language,
                    untranslated,
                    missing_plural_categories,
                }
            })
            .collect(),
        orphaned: orphaned
            .into_iter()
            .map(|((key, context), languages)| OrphanedEntry {
                key,
                context,
                languages: languages.into_iter().collect(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(key: &str, context: Option<&str>, counted: bool) -> UsedReference {
        UsedReference {
            reference: Reference {
                key: key.to_owned(),
                context: context.map(str::to_owned),
            },
            counted,
        }
    }

    fn entry(key: &str, language: &str, plural_category: &str) -> Entry {
        Entry {
            key: key.to_owned(),
            context: None,
            language: language.to_owned(),
            plural_category: plural_category.to_owned(),
            text: key.to_owned(),
        }
    }

    #[test]
    fn reports_untranslated_plural_gaps_and_orphans() {
        let report = report(
            &[
                reference("Product", None, true),
                reference("Price", None, false),
                reference("Order", Some("sorting"), false),
            ],
            &[
                entry("Product", "en", "one"),
                entry("Product", "pl", "one"),
                entry("Product", "pl", "few"),
                entry("Price", "pl", "other"),
                entry("Order", "pl", "other"),
                entry("Removed", "de", "other"),
            ],
            &[],
        );
        assert_eq!(report.reference_count, 3);
        assert_eq!(
            report
                .languages
                .iter()
                .map(|language| language.language.as_str())
                .collect::<Vec<_>>(),
            ["de", "en", "pl"]
        );
        let en = &report.languages[1];
        assert!(en.untranslated.is_empty());
        assert_eq!(
            en.missing_plural_categories,
            [MissingPluralCategories {
                key: "Product".to_owned(),
                context: None,
                missing: vec!["other"],
            }]
        );
        let pl = &report.languages[2];
        assert_eq!(pl.translated_count, 2);
        assert_eq!(
            pl.untranslated,
            [Reference {
                key: "Order".to_owned(),
                context: Some("sorting".to_owned()),
            }]
        );
        assert_eq!(pl.missing_plural_categories[0].missing, ["many", "other"]);
        assert_eq!(
            report
                .orphaned
                .iter()
                .map(|orphan| orphan.key.as_str())
                .collect::<Vec<_>>(),
            ["Order", "Removed"]
        );
    }
}
