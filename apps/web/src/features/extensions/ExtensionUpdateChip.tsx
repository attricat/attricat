import { CircleArrowUpIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { StatusChip } from '../../components/StatusChip';
import type { ExtensionUpdate } from './schemas';

/** The newer reviewed version an installed extension can upgrade to, if any. */
export const ExtensionUpdateChip = ({
  update,
}: {
  update: ExtensionUpdate | undefined;
}) => {
  const { t } = useTranslation();
  if (!update) return null;
  return (
    <StatusChip
      label={t('extensions.updateAvailable', {
        version: update.latest_version,
      })}
      size="small"
      style={{ color: 'info', icon: CircleArrowUpIcon }}
    />
  );
};
