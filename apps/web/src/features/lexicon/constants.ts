/** i18next namespace holding the workspace lexicon, kept apart from app strings. */
export const LEXICON_NAMESPACE = 'lexicon';
export const LEXICON_ENTRIES_PATH = '/api/lexicon/entries';
export const LEXICON_LANGUAGE_PARAM = 'language';
/** Plural category of entries without count-dependent forms. */
export const DEFAULT_PLURAL_CATEGORY = 'other';
export const SINGULAR_PLURAL_CATEGORY = 'one';
/** Joins a key and its context, matching i18next's context suffix. */
export const CONTEXT_SEPARATOR = '_';
/** Joins a key and its plural category, matching i18next's plural suffix. */
export const PLURAL_SEPARATOR = '_';
export const LEXICON_EXPORT_PATH = '/api/lexicon/export';
export const LEXICON_IMPORT_PATH = '/api/lexicon/import';
export const LEXICON_REPORT_PATH = '/api/lexicon/report';
export const LEXICON_REPORT_LANGUAGES_PARAM = 'languages';
export const LEXICON_FILE_FORMAT_VERSION = 1;
/** Every CLDR cardinal plural category, in CLDR order. */
export const PLURAL_CATEGORIES = [
  'zero',
  'one',
  'two',
  'few',
  'many',
  'other',
] as const;
export const LEXICON_IMPORT_MODES = ['merge', 'replace'] as const;
export type LexiconImportMode = (typeof LEXICON_IMPORT_MODES)[number];
export const LEXICON_SOURCES = {
  workspace: 'workspace',
  solutionPack: 'solution_pack',
} as const;
/** Server limits from `crates/lexicon`, mirrored for inline validation. */
export const MAX_LEXICON_KEY_LENGTH = 200;
export const MAX_LEXICON_CONTEXT_LENGTH = 100;
export const MAX_LEXICON_TEXT_LENGTH = 1000;
/** Characters a key or context cannot contain. */
export const LEXICON_RESERVED_CHARACTERS = /[{}|]/u;
export const LEXICON_FILE_ACCEPT = '.json,.toml,application/json';
export const TOML_FILE_EXTENSION = '.toml';
/** How many sample numbers to show for each plural category. */
export const PLURAL_EXAMPLE_COUNT = 3;
/** Integers scanned for plural examples; covers every CLDR rule's first hits. */
export const PLURAL_EXAMPLE_SCAN_LIMIT = 200;
/** Fractional sample for categories no integer reaches (Polish `other`). */
export const PLURAL_FRACTION_EXAMPLE = 1.5;
/** Width of the language picker and filter on the management page. */
export const LEXICON_CONTROL_WIDTH = 280;
/** Lazily loaded strings of the lexicon management page. */
export const LEXICON_MANAGEMENT_NAMESPACE = 'lexiconManagement';
/** Page strings first, then shared app strings such as `common.cancel`. */
export const LEXICON_MANAGEMENT_NAMESPACES = [
  LEXICON_MANAGEMENT_NAMESPACE,
  'translation',
] as const;
