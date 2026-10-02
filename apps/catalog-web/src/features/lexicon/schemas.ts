import { z } from 'zod';

export const lexiconEntrySchema = z.object({
  key: z.string(),
  context: z.string().nullable(),
  language: z.string(),
  plural_category: z.string(),
  text: z.string(),
});

export type LexiconEntry = z.infer<typeof lexiconEntrySchema>;

export const lexiconEntriesSchema = z.array(lexiconEntrySchema);
