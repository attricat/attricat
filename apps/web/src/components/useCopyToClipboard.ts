import { useTranslation } from 'react-i18next';
import { copyToClipboard } from './clipboard';
import { useToast } from './useToast';

/** Copies text and tells the user whether it worked. */
export const useCopyToClipboard = () => {
  const { t } = useTranslation();
  const { show } = useToast();
  return async (value: string) => {
    try {
      await copyToClipboard(value);
      show({ message: t('common.copied'), severity: 'success' });
    } catch {
      show({ message: t('common.copyFailed'), severity: 'error' });
    }
  };
};
