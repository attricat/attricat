//! Workspace lexicon: `{{key}}` / `{{key|context}}` references in catalog
//! text, translation entry validation, CLDR plural categories, and the
//! translation coverage report. The web app implements the same reference
//! grammar; both run `contracts/lexicon-references.json`.

mod entry;
mod plural;
mod reference;
mod report;

pub use entry::{
    Entry, EntryError, EntryIdentity, LEXICON_FILE_FORMAT_VERSION, LexiconFile, LexiconFileEntry,
    MAX_CONTEXT_CHARS, MAX_FILE_ENTRIES, MAX_KEY_CHARS, MAX_TEXT_CHARS, lexicon_file_json_schema,
};
pub use plural::{
    DEFAULT_PLURAL_CATEGORY, PLURAL_CATEGORIES, canonical_language, plural_categories,
    supported_languages,
};
pub use reference::{Reference, ReferenceError, Segment, normalize, parse, references, resolve};
pub use report::{
    LanguageReport, MissingPluralCategories, OrphanedEntry, Report, SOURCE_LANGUAGE, UsedReference,
    report,
};
