import { useTranslation } from 'react-i18next';
import type { ExtensionContribution } from './api';

type Titled = Pick<ExtensionContribution, 'id' | 'title' | 'translations'>;

/**
 * A contribution's title in `language` (or its base language), falling back
 * to the manifest's own title and then its ID.
 */
export const contributionTitle = (contribution: Titled, language: string) =>
  contribution.translations[language]?.title ??
  contribution.translations[language.split('-')[0]]?.title ??
  contribution.title ??
  contribution.id;

/** {@link contributionTitle} in the viewer's language. */
export const useContributionTitle = () => {
  const { i18n } = useTranslation();
  return (contribution: Titled) =>
    contributionTitle(contribution, i18n.language);
};
