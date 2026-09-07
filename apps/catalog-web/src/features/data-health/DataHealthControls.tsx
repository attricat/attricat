import {
  Alert,
  Button,
  FormControl,
  InputLabel,
  MenuItem,
  Select,
  Stack,
  TextField,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

type Props = {
  staleAfterDays: number;
  isRefreshing: boolean;
  refreshError: Error | null;
  onStaleAfterDaysChange: (days: number) => void;
  onRefresh: () => void;
};

export const DataHealthControls = (props: Props) => (
  <DataHealthControlsForm key={props.staleAfterDays} {...props} />
);

const DataHealthControlsForm = ({
  staleAfterDays,
  isRefreshing,
  refreshError,
  onStaleAfterDaysChange,
  onRefresh,
}: Props) => {
  const { t } = useTranslation();
  const [customThreshold, setCustomThreshold] = useState(
    ![30, 90, 180, 365].includes(staleAfterDays),
  );

  return (
    <>
      <Stack
        direction={{ xs: 'column', sm: 'row' }}
        spacing={2}
        sx={{ alignItems: { sm: 'center' } }}
      >
        <FormControl size="small" sx={{ minWidth: 160 }}>
          <InputLabel id="stale-after-label">
            {t('dataHealth.staleAfter')}
          </InputLabel>
          <Select
            label={t('dataHealth.staleAfter')}
            labelId="stale-after-label"
            value={customThreshold ? 'custom' : staleAfterDays}
            onChange={(event) => {
              if (event.target.value === 'custom') {
                setCustomThreshold(true);
                return;
              }
              setCustomThreshold(false);
              onStaleAfterDaysChange(Number(event.target.value));
            }}
          >
            {[30, 90, 180, 365].map((days) => (
              <MenuItem key={days} value={days}>
                {t('dataHealth.daysCount', { count: days })}
              </MenuItem>
            ))}
            <MenuItem value="custom">{t('dataHealth.custom')}</MenuItem>
          </Select>
        </FormControl>
        {customThreshold && (
          <TextField
            defaultValue={staleAfterDays}
            label={t('dataHealth.days')}
            onBlur={(event) => {
              const days = Number(event.target.value);
              if (Number.isInteger(days) && days >= 1 && days <= 3650)
                onStaleAfterDaysChange(days);
            }}
            size="small"
            slotProps={{ htmlInput: { min: 1, max: 3650 } }}
            type="number"
            sx={{ width: 110 }}
          />
        )}
        <Button disabled={isRefreshing} onClick={onRefresh} variant="outlined">
          {isRefreshing ? t('dataHealth.refreshing') : t('dataHealth.refresh')}
        </Button>
      </Stack>
      {refreshError && (
        <Alert severity="error" sx={{ mt: 2 }}>
          {refreshError.message}
        </Alert>
      )}
    </>
  );
};
