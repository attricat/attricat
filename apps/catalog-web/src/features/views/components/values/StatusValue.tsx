import { Chip, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { StatusConfiguration } from '../../../entities/status';

export const StatusValue = ({
  config,
  value,
}: {
  config: StatusConfiguration;
  value: unknown;
}) => {
  const { t } = useTranslation();
  if (value === null || value === undefined)
    return (
      <Typography color="text.secondary" variant="body2">
        {t('views.notSet')}
      </Typography>
    );
  const option = config.options.find((item) => item.code === value);
  const label = option?.label ?? String(value);
  return (
    <Stack spacing={0.5} sx={{ alignItems: 'flex-start' }}>
      <Chip
        label={label}
        color={option?.tone ?? 'default'}
        variant="outlined"
        sx={{
          maxWidth: '100%',
          height: 'auto',
          '& .MuiChip-label': {
            whiteSpace: 'normal',
            overflowWrap: 'anywhere',
          },
        }}
      />
      {!option && (
        <Typography color="text.secondary" variant="caption">
          {t('entities.statusUnknown', { value: label })}
        </Typography>
      )}
    </Stack>
  );
};
