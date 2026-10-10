import { Box, Chip, Tooltip } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { RouterButton } from '../../../components/RouterLink';
import { BlueprintIcon } from '../../../components/systemIcons';
import { lexiconText } from '../../lexicon/lexicon';
import { RecordSchemaStatus } from './RecordSchemaStatus';

type Props = {
  blueprint?: { id: string; name: string };
  children?: ReactNode;
  /** Omits the link to the blueprint, such as where a panel already names it. */
  hideBlueprintLink?: boolean;
  isSample?: boolean;
  /** Shows whether the record matches the blueprint's current schema. */
  schemaOutdated?: boolean;
};

/** Page header actions identifying a record's blueprint and sample state. */
export const RecordBlueprintHeaderActions = ({
  blueprint,
  children,
  hideBlueprintLink = false,
  isSample = false,
  schemaOutdated,
}: Props) => {
  const { t } = useTranslation();
  return (
    <Box
      sx={{ alignItems: 'center', display: 'flex', flexWrap: 'wrap', gap: 1 }}
    >
      {children}
      {isSample && (
        <Chip color="info" label={t('records.sample')} size="small" />
      )}
      {blueprint && !hideBlueprintLink && (
        <Tooltip title={lexiconText(blueprint.name)}>
          <RouterButton
            params={{ blueprintId: blueprint.id }}
            size="small"
            startIcon={<BlueprintIcon />}
            to="/manage/blueprints/$blueprintId"
            variant="text"
          >
            {t('records.blueprintLabel', {
              name: lexiconText(blueprint.name),
            })}
          </RouterButton>
        </Tooltip>
      )}
      {blueprint && schemaOutdated !== undefined && (
        <RecordSchemaStatus schemaOutdated={schemaOutdated} />
      )}
    </Box>
  );
};
