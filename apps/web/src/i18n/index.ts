import i18n, { type BackendModule } from 'i18next';
import LanguageDetector from 'i18next-browser-languagedetector';
import { initReactI18next } from 'react-i18next';
import {
  isLexiconNamespace,
  lexiconBackend,
} from '../features/lexicon/lexicon';
import { localeBackend } from './localeBackend';
// Schema failures read as translated sentences once the app loads strings.
import './zodErrors';

export const supportedLanguages = ['en', 'pl'] as const;
export type SupportedLanguage = (typeof supportedLanguages)[number];
export const defaultLanguage: SupportedLanguage = 'en';
/**
 * The language the person picked with the switcher. Before 1.9 the detected
 * browser language was stored under `attricat.language` on every visit, so
 * that key cannot tell a pick from a guess and is no longer read.
 */
export const languageStorageKey = 'attricat.language-choice';

// i18next takes one backend: app strings load per language, workspace
// lexicon namespaces come from the lexicon backend.
const backend: BackendModule = {
  type: 'backend',
  // `read` answers through its callback.
  callbackApi: true,
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
    // Only a language the person picks is stored, so the installation
    // default still applies to everyone who has not picked one.
    detection: {
      caches: [],
      lookupLocalStorage: languageStorageKey,
      order: ['localStorage', 'navigator'],
    },
  });

const storedLanguage = () => {
  try {
    return localStorage.getItem(languageStorageKey);
  } catch {
    return null;
  }
};

/** Shows the app in the person's pick and remembers it in this browser. */
export const chooseLanguage = async (language: SupportedLanguage) => {
  try {
    localStorage.setItem(languageStorageKey, language);
  } catch {
    // Without storage the pick lasts until the page reloads.
  }
  await i18n.changeLanguage(language);
};

/**
 * Applies the installation's default language over the browser's, unless
 * the person picked one in this browser.
 */
export const applyInstallationLanguage = async (
  language: SupportedLanguage | null,
) => {
  if (!language || storedLanguage()) return;
  await i18n.changeLanguage(language);
};

/** The supported language the app is shown in, falling back to the default. */
export const activeLanguage = (
  resolved: string | undefined = i18n.resolvedLanguage,
): SupportedLanguage =>
  supportedLanguages.find((code) => code === resolved) ?? defaultLanguage;

export default i18n;
