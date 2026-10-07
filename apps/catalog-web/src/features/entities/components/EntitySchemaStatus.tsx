import { Tooltip, useTheme } from '@mui/material';
import { CircleCheckIcon, TriangleAlertIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { smallIconSize } from '../../../components/iconSizes';

type Props = {
  schemaOutdated: boolean;
};

/**
 * Shows whether an entity matches its blueprint's current schema. The upgrade
 * itself is offered in the entity actions menu.
 */
export const EntitySchemaStatus = ({ schemaOutdated }: Props) => {
  const { t } = useTranslation();
  const { palette } = useTheme();
  return schemaOutdated ? (
    <Tooltip title={t('entities.schemaOutdated')}>
      <TriangleAlertIcon
        aria-label={t('entities.schemaOutdated')}
        color={palette.warning.main}
        role="img"
        size={smallIconSize}
      />
    </Tooltip>
  ) : (
    <Tooltip title={t('entities.matchesCurrentSchema')}>
      <CircleCheckIcon
        aria-label={t('entities.matchesCurrentSchema')}
        color={palette.success.main}
        role="img"
        size={smallIconSize}
      />
    </Tooltip>
  );
};
