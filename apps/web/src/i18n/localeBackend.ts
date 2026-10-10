import type { BackendModule } from 'i18next';

const locales = {
  en: () => import('./locales/en.json'),
  pl: () => import('./locales/pl.json'),
};

/** Locale chunks are fetched only when i18next needs that language. */
export const localeBackend: BackendModule = {
  type: 'backend',
  init: () => undefined,
  read: (language, _namespace, callback) => {
    const load = locales[language as keyof typeof locales];
    if (!load) {
      callback(new Error(`Unsupported language: ${language}`), false);
      return;
    }
    void load().then(
      ({ default: messages }) => callback(null, messages),
      (error: unknown) =>
        callback(
          error instanceof Error ? error : new Error(String(error)),
          false,
        ),
    );
  },
};
