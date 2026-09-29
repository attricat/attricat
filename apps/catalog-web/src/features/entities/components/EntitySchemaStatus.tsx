import { Tooltip, useTheme } from '@mui/material';
import {
  CircleArrowUpIcon,
  CircleCheckIcon,
  TriangleAlertIcon,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { RouterIconButton } from '../../../components/RouterLink';
import { smallIconSize } from '../../../components/iconSizes';

type Props = {
  entityId: string;
  schemaOutdated: boolean;
};

/** Shows whether an entity matches its blueprint's current schema. */
export const EntitySchemaStatus = ({ entityId, schemaOutdated }: Props) => {
  const { t } = useTranslation();
  const { palette } = useTheme();
  if (!schemaOutdated)
    return (
      <Tooltip title={t('entities.matchesCurrentSchema')}>
        <CircleCheckIcon color={palette.success.main} size={smallIconSize} />
      </Tooltip>
    );
  return (
    <>
      <Tooltip title={t('entities.schemaOutdated')}>
        <TriangleAlertIcon color={palette.warning.main} size={smallIconSize} />
      </Tooltip>
      <Tooltip title={t('entities.upgradeBlueprint')}>
        <RouterIconButton
          aria-label={t('entities.upgradeBlueprint')}
          params={{ entityId }}
          to="/entities/$entityId/migrate"
        >
          <CircleArrowUpIcon />
        </RouterIconButton>
      </Tooltip>
    </>
  );
};
