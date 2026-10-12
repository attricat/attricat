import { Chip, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { BlueprintLink } from '../blueprints/BlueprintLink';

export const BlueprintLabel = ({
  code,
  currentVersion,
  id,
  name,
  outdatedRecords,
}: {
  code: string;
  currentVersion: number | string;
  id: string;
  name: string;
  outdatedRecords: number;
}) => {
  const { t } = useTranslation();

  return (
    <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
      <Typography variant="body2">
        <BlueprintLink blueprint={{ id, name }} /> ({code}) v{currentVersion}
      </Typography>
      {outdatedRecords > 0 && (
        <Chip
          color="warning"
          label={t('dataHealth.outdatedCount', { count: outdatedRecords })}
          size="small"
        />
      )}
    </Stack>
  );
};
