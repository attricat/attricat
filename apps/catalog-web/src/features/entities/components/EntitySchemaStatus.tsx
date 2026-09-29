import CheckCircleOutlinedIcon from '@mui/icons-material/CheckCircleOutlined';
import UpgradeOutlinedIcon from '@mui/icons-material/UpgradeOutlined';
import WarningAmberOutlinedIcon from '@mui/icons-material/WarningAmberOutlined';
import { Tooltip } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { RouterIconButton } from '../../../components/RouterLink';

type Props = {
  entityId: string;
  schemaOutdated: boolean;
};

/** Shows whether an entity matches its blueprint's current schema. */
export const EntitySchemaStatus = ({ entityId, schemaOutdated }: Props) => {
  const { t } = useTranslation();
  if (!schemaOutdated)
    return (
      <Tooltip title={t('entities.matchesCurrentSchema')}>
        <CheckCircleOutlinedIcon color="success" fontSize="small" />
      </Tooltip>
    );
  return (
    <>
      <Tooltip title={t('entities.schemaOutdated')}>
        <WarningAmberOutlinedIcon color="warning" fontSize="small" />
      </Tooltip>
      <Tooltip title={t('entities.upgradeBlueprint')}>
        <RouterIconButton
          aria-label={t('entities.upgradeBlueprint')}
          params={{ entityId }}
          to="/entities/$entityId/migrate"
        >
          <UpgradeOutlinedIcon />
        </RouterIconButton>
      </Tooltip>
    </>
  );
};
