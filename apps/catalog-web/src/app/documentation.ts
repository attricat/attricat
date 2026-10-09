import { useTranslation } from 'react-i18next';
import type { SupportedLanguage } from '../i18n';

export const documentationSiteUrl = 'https://docs.attricat.com';

// Paths of public documentation pages, relative to a locale root.
export const documentationPages = {
  home: '',
  blueprints: 'builders/blueprints/',
  contexts: 'guides/contexts/',
  searchSyntax: 'guides/search-syntax/',
  extensions: 'builders/extensions/',
  workspaces: 'operate/workspaces/',
} as const;

export type DocumentationPage = keyof typeof documentationPages;

const documentationLocalePrefixes: Record<SupportedLanguage, string> = {
  en: '',
  pl: 'pl/',
};

const isSupportedLanguage = (
  language: string | undefined,
): language is SupportedLanguage =>
  language !== undefined && language in documentationLocalePrefixes;

export const documentationUrl = (
  page: DocumentationPage,
  language?: string,
) => {
  const prefix = isSupportedLanguage(language)
    ? documentationLocalePrefixes[language]
    : '';
  return `${documentationSiteUrl}/${prefix}${documentationPages[page]}`;
};

export const useDocumentationUrl = () => {
  const { i18n } = useTranslation();
  return (page: DocumentationPage) =>
    documentationUrl(page, i18n.resolvedLanguage);
};
