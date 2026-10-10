import { Tooltip, useTheme } from '@mui/material';
import { CircleCheckIcon, TriangleAlertIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { smallIconSize } from '../../../components/iconSizes';

type Props = {
  schemaOutdated: boolean;
};

/**
 * Shows whether a record matches its blueprint's current schema. The upgrade
 * itself is offered in the record actions menu.
 */
export const RecordSchemaStatus = ({ schemaOutdated }: Props) => {
  const { t } = useTranslation();
  const { palette } = useTheme();
  return schemaOutdated ? (
    <Tooltip title={t('records.schemaOutdated')}>
      <TriangleAlertIcon
        aria-label={t('records.schemaOutdated')}
        color={palette.warning.main}
        role="img"
        size={smallIconSize}
      />
    </Tooltip>
  ) : (
    <Tooltip title={t('records.matchesCurrentSchema')}>
      <CircleCheckIcon
        aria-label={t('records.matchesCurrentSchema')}
        color={palette.success.main}
        role="img"
        size={smallIconSize}
      />
    </Tooltip>
  );
};
