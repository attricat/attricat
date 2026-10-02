import { Chip, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { lexiconText } from '../lexicon/lexicon';

export const BlueprintLabel = ({
  code,
  currentVersion,
  name,
  outdatedEntities,
}: {
  code: string;
  currentVersion: number | string;
  name: string;
  outdatedEntities: number;
}) => {
  const { t } = useTranslation();

  return (
    <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
      <Typography variant="body2">
        {lexiconText(name)} ({code}) v{currentVersion}
      </Typography>
      {outdatedEntities > 0 && (
        <Chip
          color="warning"
          label={t('dataHealth.outdatedCount', { count: outdatedEntities })}
          size="small"
        />
      )}
    </Stack>
  );
};
