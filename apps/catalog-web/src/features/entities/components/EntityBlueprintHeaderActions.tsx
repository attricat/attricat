import { Box, Chip, Tooltip } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { RouterButton } from '../../../components/RouterLink';
import { BlueprintIcon } from '../../../components/systemIcons';
import { lexiconText } from '../../lexicon/lexicon';
import { EntitySchemaStatus } from './EntitySchemaStatus';

type Props = {
  blueprint?: { id: string; name: string };
  children?: ReactNode;
  isSample?: boolean;
  /** Shows whether the entity matches the blueprint's current schema. */
  schemaOutdated?: boolean;
};

/** Page header actions identifying an entity's blueprint and sample state. */
export const EntityBlueprintHeaderActions = ({
  blueprint,
  children,
  isSample = false,
  schemaOutdated,
}: Props) => {
  const { t } = useTranslation();
  return (
    <Box sx={{ alignItems: 'center', display: 'flex', gap: 1 }}>
      {children}
      {isSample && (
        <Chip color="info" label={t('entities.sample')} size="small" />
      )}
      {blueprint && (
        <Tooltip title={lexiconText(blueprint.name)}>
          <RouterButton
            params={{ blueprintId: blueprint.id }}
            size="small"
            startIcon={<BlueprintIcon />}
            to="/manage/blueprints/$blueprintId"
            variant="text"
          >
            {t('entities.blueprintLabel', {
              name: lexiconText(blueprint.name),
            })}
          </RouterButton>
        </Tooltip>
      )}
      {blueprint && schemaOutdated !== undefined && (
        <EntitySchemaStatus schemaOutdated={schemaOutdated} />
      )}
    </Box>
  );
};
