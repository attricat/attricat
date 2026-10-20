import { ExternalLinkIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { captionIconSize } from './iconSizes';

/**
 * Marks a link that opens in a new tab, ahead of its text, and tells screen
 * reader users the same through its accessible name.
 */
export const NewTabIcon = ({ size = captionIconSize }: { size?: number }) => {
  const { t } = useTranslation();
  return (
    <ExternalLinkIcon
      aria-label={t('common.opensInNewTab')}
      role="img"
      size={size}
    />
  );
};
