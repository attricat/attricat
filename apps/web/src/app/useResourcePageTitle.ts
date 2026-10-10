import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

/** Replace the route's fallback label once its resource name is available. */
export const useResourcePageTitle = (
  name: string | undefined,
  section?: string,
) => {
  const { t } = useTranslation();
  useEffect(() => {
    if (name) {
      document.title = `${section ? `${section} · ` : ''}${name} · ${t('app.attricat')}`;
    }
  }, [name, section, t]);
};
