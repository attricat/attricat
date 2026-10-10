import i18n, { type BackendModule } from 'i18next';
import LanguageDetector from 'i18next-browser-languagedetector';
import { initReactI18next } from 'react-i18next';
import {
  isLexiconNamespace,
  lexiconBackend,
} from '../features/lexicon/lexicon';
import { localeBackend } from './localeBackend';

export const supportedLanguages = ['en', 'pl'] as const;
export type SupportedLanguage = (typeof supportedLanguages)[number];
export const defaultLanguage: SupportedLanguage = 'en';
const languageStorageKey = 'attricat.language';

// i18next takes one backend: app strings load per language, workspace
// lexicon namespaces come from the lexicon backend.
const backend: BackendModule = {
  type: 'backend',
  init: () => undefined,
  read: (language, namespace, callback) =>
    (isLexiconNamespace(namespace) ? lexiconBackend : localeBackend).read(
      language,
      namespace,
      callback,
    ),
};

export const i18nReady = i18n
  .use(backend)
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    load: 'languageOnly',
    fallbackLng: defaultLanguage,
    supportedLngs: supportedLanguages,
    interpolation: { escapeValue: false },
    // Re-render when the workspace lexicon (re)loads, not only on language change.
    react: { bindI18n: 'languageChanged loaded' },
    detection: {
      caches: ['localStorage'],
      lookupLocalStorage: languageStorageKey,
      order: ['localStorage', 'navigator'],
    },
  });

export default i18n;
