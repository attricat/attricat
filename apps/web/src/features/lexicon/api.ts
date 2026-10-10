import { request, requestNoContent } from '../../api/request';
import {
  LEXICON_ENTRIES_PATH,
  LEXICON_EXPORT_PATH,
  LEXICON_IMPORT_PATH,
  LEXICON_LANGUAGE_PARAM,
  LEXICON_REPORT_LANGUAGES_PARAM,
  LEXICON_REPORT_PATH,
  type LexiconImportMode,
} from './constants';
import {
  lexiconEntriesSchema,
  lexiconFileSchema,
  lexiconImportSummarySchema,
  lexiconReportSchema,
  storedLexiconEntrySchema,
  type LexiconEntry,
  type LexiconFile,
} from './schemas';

const query = (params: Record<string, string>) =>
  new URLSearchParams(params).toString();

const json = (method: string, body: unknown): RequestInit => ({
  method,
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify(body),
});

/** One language's translations, for the i18next lexicon namespace. */
export const listLexiconEntries = (language: string) =>
  request(
    `${LEXICON_ENTRIES_PATH}?${query({ [LEXICON_LANGUAGE_PARAM]: language })}`,
    lexiconEntriesSchema,
  );

/** Every stored entry with its source, for lexicon management. */
export const listStoredLexiconEntries = (signal?: AbortSignal) =>
  request(
    LEXICON_ENTRIES_PATH,
    storedLexiconEntrySchema.array(),
    signal === undefined ? undefined : { signal },
  );

export const getLexiconReport = (language: string, signal?: AbortSignal) =>
  request(
    `${LEXICON_REPORT_PATH}?${query({ [LEXICON_REPORT_LANGUAGES_PARAM]: language })}`,
    lexiconReportSchema,
    signal === undefined ? undefined : { signal },
  );

export const saveLexiconEntry = (entry: LexiconEntry) =>
  request(LEXICON_ENTRIES_PATH, storedLexiconEntrySchema, json('PUT', entry));

export const deleteLexiconEntry = (
  entry: Pick<LexiconEntry, 'key' | 'context' | 'language' | 'plural_category'>,
) =>
  requestNoContent(
    `${LEXICON_ENTRIES_PATH}?${query({
      key: entry.key,
      ...(entry.context ? { context: entry.context } : {}),
      language: entry.language,
      plural_category: entry.plural_category,
    })}`,
    { method: 'DELETE' },
  );

export const exportLexicon = (language: string) =>
  request(
    `${LEXICON_EXPORT_PATH}?${query({ [LEXICON_LANGUAGE_PARAM]: language })}`,
    lexiconFileSchema,
  );

export const importLexicon = (file: LexiconFile, mode: LexiconImportMode) =>
  request(
    `${LEXICON_IMPORT_PATH}?${query({ mode })}`,
    lexiconImportSummarySchema,
    json('POST', file),
  );
