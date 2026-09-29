import i18n from 'i18next';
import LanguageDetector from 'i18next-browser-languagedetector';
import { initReactI18next } from 'react-i18next';
import en from './locales/en.json';
import pl from './locales/pl.json';

export const supportedLanguages = ['en', 'pl'] as const;
export type SupportedLanguage = (typeof supportedLanguages)[number];
export const defaultLanguage: SupportedLanguage = 'en';
const languageStorageKey = 'catalog.language';

void i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: {
      en: { translation: en },
      pl: { translation: pl },
    },
    fallbackLng: defaultLanguage,
    supportedLngs: supportedLanguages,
    interpolation: { escapeValue: false },
    detection: {
      caches: ['localStorage'],
      lookupLocalStorage: languageStorageKey,
      order: ['localStorage', 'navigator'],
    },
  });

export default i18n;
