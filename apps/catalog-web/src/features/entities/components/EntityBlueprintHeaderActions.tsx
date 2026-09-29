import { Box, Chip, Tooltip } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { RouterButton } from '../../../components/RouterLink';
import { BlueprintIcon } from '../../../components/systemIcons';

type Props = {
  blueprint?: { id: string; name: string };
  children?: ReactNode;
  isSample?: boolean;
};

/** Page header actions identifying an entity's blueprint and sample state. */
export const EntityBlueprintHeaderActions = ({
  blueprint,
  children,
  isSample = false,
}: Props) => {
  const { t } = useTranslation();
  return (
    <Box sx={{ alignItems: 'center', display: 'flex', gap: 1 }}>
      {children}
      {isSample && (
        <Chip color="info" label={t('entities.sample')} size="small" />
      )}
      {blueprint && (
        <Tooltip title={blueprint.name}>
          <RouterButton
            params={{ blueprintId: blueprint.id }}
            size="small"
            startIcon={<BlueprintIcon />}
            to="/manage/blueprints/$blueprintId"
            variant="text"
          >
            {t('entities.blueprintLabel', { name: blueprint.name })}
          </RouterButton>
        </Tooltip>
      )}
    </Box>
  );
};
