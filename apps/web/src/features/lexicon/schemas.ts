import { z } from 'zod';
import { LEXICON_FILE_FORMAT_VERSION, PLURAL_CATEGORIES } from './constants';

export const lexiconEntrySchema = z.object({
  key: z.string(),
  context: z.string().nullable(),
  language: z.string(),
  plural_category: z.string(),
  text: z.string(),
});

export type LexiconEntry = z.infer<typeof lexiconEntrySchema>;

export const lexiconEntriesSchema = z.array(lexiconEntrySchema);

export const storedLexiconEntrySchema = lexiconEntrySchema.extend({
  source: z.string(),
  solution_pack_id: z.string().nullable(),
  updated_at: z.string(),
});

export type StoredLexiconEntry = z.infer<typeof storedLexiconEntrySchema>;

const referenceSchema = z.object({
  key: z.string(),
  context: z.string().nullable(),
});

export type LexiconReportReference = z.infer<typeof referenceSchema>;

export const lexiconReportSchema = z.object({
  reference_count: z.number(),
  languages: z.array(
    z.object({
      language: z.string(),
      translated_count: z.number(),
      untranslated: z.array(referenceSchema),
      missing_plural_categories: z.array(
        referenceSchema.extend({ missing: z.array(z.string()) }),
      ),
    }),
  ),
  orphaned: z.array(referenceSchema.extend({ languages: z.array(z.string()) })),
});

export type LexiconReport = z.infer<typeof lexiconReportSchema>;

export const lexiconImportSummarySchema = z.object({
  created: z.number(),
  updated: z.number(),
  unchanged: z.number(),
  deleted: z.number(),
});

/** `contracts/lexicon-v1.schema.json`; the server re-validates every entry. */
export const lexiconFileSchema = z.object({
  format_version: z.literal(LEXICON_FILE_FORMAT_VERSION),
  language: z.string().min(1),
  entries: z.array(
    z.object({
      key: z.string(),
      context: z.string().optional(),
      plural_category: z.enum(PLURAL_CATEGORIES).optional(),
      text: z.string(),
    }),
  ),
});

export type LexiconFile = z.infer<typeof lexiconFileSchema>;
