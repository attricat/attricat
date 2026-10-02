export const lexiconQueryKeys = {
  all: () => ['lexicon'] as const,
  entries: () => [...lexiconQueryKeys.all(), 'entries'] as const,
  report: (language: string) =>
    [...lexiconQueryKeys.all(), 'report', language] as const,
} as const;
