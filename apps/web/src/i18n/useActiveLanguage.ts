import { useTranslation } from 'react-i18next';
import { activeLanguage, type SupportedLanguage } from '.';

/** The supported language the app is shown in, falling back to the default. */
export const useActiveLanguage = (): SupportedLanguage => {
  const { i18n } = useTranslation();
  return activeLanguage(i18n.resolvedLanguage);
};
